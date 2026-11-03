//! The network signals of `catalogue tiers`: repo facts and app distribution counts.

use std::collections::BTreeMap;
use std::path::Path;

use catalogue_core::{Catalogue, Date, Entry, EntryKind, Slug, normalize_repo_url};
use catalogue_signals::distribution::{
    HOMEBREW_INTERVAL, RELEASES_INTERVAL, REPOLOGY_INTERVAL, homebrew_installs, release_downloads,
    repology_packaging,
};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::fallback::{
    Failed, FetchFailure, FetchSummary, Source, attempt, distribution_failure, github_failure,
    recover,
};
use catalogue_signals::github::{GITHUB_INTERVAL, RepoRef, repo_facts};
use catalogue_signals::history::{Dated, History, NetworkValues, ReleaseValues, RepoValues};
use catalogue_signals::http::{IntervalPacer, Transport};

use super::docker::fetch_docker;
use super::error::TiersError;
use super::links::fetch_links;
use crate::batch;

/// Where the network signals come from.
#[derive(Clone, Copy)]
pub enum Network<'a> {
    /// No request is sent. Every network signal reads as unknown.
    Offline,
    /// Requests go through `transport`. GitHub requests send `token`.
    Online {
        /// The HTTP transport.
        transport: &'a dyn Transport,
        /// The GitHub token.
        token: &'a str,
        /// The cache directory. Its `links` directory holds the repo link results.
        cache_dir: &'a Path,
    },
}

/// The network signals of one entry, with a note for each source that had no answer.
#[derive(Debug, Default)]
pub(crate) struct Fetched {
    /// Each value with the date of its data: the run date for a fresh fetch, the
    /// stored date for a cached link check or a fallback.
    pub(crate) values: NetworkValues,
    pub(crate) notes: Vec<String>,
}

/// What the network phase of a run hands on.
#[derive(Debug, Default)]
pub(crate) struct Collected {
    /// The network signals by slug, with a record for every entry.
    pub(crate) entries: BTreeMap<Slug, Fetched>,
    /// The fetches of the run and the ones that failed.
    pub(crate) summary: FetchSummary,
}

/// The field of [`NetworkValues`] that one source fills.
type Slot<T> = for<'a> fn(&'a mut NetworkValues) -> &'a mut Option<Dated<T>>;

/// One fetch of one entry: the source, what it asks about and where its value goes.
struct Fetch<'a, T> {
    slug: &'a Slug,
    source: Source,
    of: &'a str,
    slot: Slot<T>,
}

/// Collects fetch results. A failed fetch reads the snapshots in `history`.
struct Collector<'a> {
    history: &'a History,
    max_age_days: u32,
    today: Date,
    out: Collected,
}

impl Collector<'_> {
    fn slot(&mut self, slug: &Slug) -> &mut Fetched {
        self.out.entries.entry(slug.clone()).or_default()
    }

    fn note(&mut self, slug: &Slug, note: String) {
        self.slot(slug).notes.push(note);
    }

    /// Stores a value that the source supplied.
    fn found<T>(&mut self, fetch: &Fetch<'_, T>, date: Date, value: T) {
        let dated = Dated {
            date,
            of: fetch.of.to_owned(),
            value,
        };
        *(fetch.slot)(&mut self.slot(fetch.slug).values) = Some(dated);
    }

    /// Replaces a failed fetch with the value of the previous snapshot, inside
    /// the age limit, and records the failure for the run summary.
    fn fell_back<T: Clone>(&mut self, fetch: &Fetch<'_, T>, error: &str) {
        let today = self.today;
        let mut earlier = self
            .history
            .previous_network(fetch.slug.as_str(), today)
            .cloned()
            .unwrap_or_default();
        let earlier = (fetch.slot)(&mut earlier).take();
        let found = recover(
            fetch.source,
            earlier,
            fetch.of,
            error,
            today,
            self.max_age_days,
        );
        self.out.summary.failures.push(FetchFailure {
            slug: fetch.slug.clone(),
            source: fetch.source,
            from: found.value.as_ref().map(|value| value.date),
        });
        let fetched = self.slot(fetch.slug);
        *(fetch.slot)(&mut fetched.values) = found.value;
        fetched.notes.push(found.note);
    }

    /// Settles one fetch dated `today`. `absent` is the note for a source that
    /// answers it does not know the name.
    fn settle<T: Clone>(
        &mut self,
        fetch: &Fetch<'_, T>,
        result: Result<Option<T>, Failed>,
        absent: impl FnOnce() -> String,
    ) {
        match result {
            Ok(Some(value)) => self.found(fetch, self.today, value),
            Ok(None) => self.note(fetch.slug, absent()),
            Err(Failed::Definitive(error)) => self.note(fetch.slug, error),
            Err(Failed::Fallback(error)) => self.fell_back(fetch, &error),
        }
    }
}

/// Fetches the network signals of every entry, keyed by slug, with the run summary.
///
/// Every entry with a GitHub repo gets its repo facts. Every app with a GitHub
/// repo gets its release downloads, then the Homebrew and Repology counts its
/// `distribution` names, then the Docker Hub pulls of its listed images. Every
/// library gets its repo link check, which `snapshot` supplies the release for.
/// Each source has its own pacer.
///
/// [`catalogue_signals::fallback`] sorts every error. A fatal error ends the run.
/// A definite "no such name" becomes a note. Any other error reuses that source's
/// value from the newest snapshot in `history` that holds the entry, up to
/// `max_age_days` old, and otherwise leaves the signal unknown. Only these
/// network fetches fall back: the dump and the RustSec archive have their own files.
pub(crate) fn fetch_all(
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    network: Network<'_>,
    history: &History,
    max_age_days: u32,
    today: Date,
) -> Result<Collected, TiersError> {
    let mut run = Collector {
        history,
        max_age_days,
        today,
        out: Collected {
            entries: catalogue
                .entries()
                .keys()
                .map(|slug| (slug.clone(), Fetched::default()))
                .collect(),
            summary: FetchSummary::default(),
        },
    };
    let Network::Online {
        transport,
        token,
        cache_dir,
    } = network
    else {
        return Ok(run.out);
    };
    let mut repos: Vec<(Slug, RepoRef)> = Vec::new();
    for (slug, entry) in catalogue.entries() {
        match github_repo(entry) {
            Ok(repo) => repos.push((slug.clone(), repo)),
            Err(note) => run.note(slug, note),
        }
    }
    collect_facts(&mut run, &repos, transport, token)?;
    collect_links(&mut run, catalogue, snapshot, transport, token, cache_dir)?;
    let apps: Vec<(Slug, RepoRef)> = repos
        .into_iter()
        .filter(|(slug, _)| kind(catalogue, slug) == Some(EntryKind::App))
        .collect();
    collect_releases(&mut run, &apps, transport, token)?;
    collect_homebrew(&mut run, catalogue, transport)?;
    collect_repology(&mut run, catalogue, transport)?;
    collect_docker(&mut run, catalogue, transport)?;
    run.out.summary.failures.sort();
    Ok(run.out)
}

fn collect_facts(
    run: &mut Collector<'_>,
    repos: &[(Slug, RepoRef)],
    transport: &dyn Transport,
    token: &str,
) -> Result<(), TiersError> {
    let today = run.today.civil();
    let mut pacer = IntervalPacer::new(GITHUB_INTERVAL);
    let found = batch::each(repos, &mut pacer, |(_, repo), pacer| {
        attempt(
            repo_facts(transport, token, repo, today, pacer),
            github_failure,
        )
    })?;
    run.out.summary.fetches += repos.len();
    for ((slug, repo), result) in repos.iter().zip(found) {
        let of = repo.to_string();
        let fetch = Fetch {
            slug,
            source: Source::RepoFacts,
            of: &of,
            slot: |values| &mut values.facts,
        };
        let result = result.map(|facts| Some(RepoValues::from(&facts)));
        run.settle(&fetch, result, String::new);
    }
    Ok(())
}

fn collect_links(
    run: &mut Collector<'_>,
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    transport: &dyn Transport,
    token: &str,
    cache_dir: &Path,
) -> Result<(), TiersError> {
    let links = fetch_links(catalogue, snapshot, transport, token, cache_dir, run.today)?;
    run.out.summary.fetches += links.fetches;
    for (slug, check) in links.checks {
        run.slot(&slug).values.link = Some(check);
    }
    for (slug, note) in links.notes {
        run.note(&slug, note);
    }
    for (slug, failure) in links.failures {
        let fetch = Fetch {
            slug: &slug,
            source: Source::RepoLink,
            of: &failure.of,
            slot: |values| &mut values.link,
        };
        run.fell_back(&fetch, &failure.error);
    }
    Ok(())
}

fn collect_releases(
    run: &mut Collector<'_>,
    apps: &[(Slug, RepoRef)],
    transport: &dyn Transport,
    token: &str,
) -> Result<(), TiersError> {
    let mut pacer = IntervalPacer::new(RELEASES_INTERVAL);
    let found = batch::each(apps, &mut pacer, |(_, repo), pacer| {
        attempt(release_downloads(transport, token, repo, pacer), |error| {
            distribution_failure(error, Source::Releases)
        })
    })?;
    run.out.summary.fetches += apps.len();
    for ((slug, repo), result) in apps.iter().zip(found) {
        let of = repo.to_string();
        let fetch = Fetch {
            slug,
            source: Source::Releases,
            of: &of,
            slot: |values| &mut values.releases,
        };
        let result = result.map(|found| found.as_ref().map(ReleaseValues::from));
        run.settle(&fetch, result, || {
            format!("GitHub has no releases for {repo}")
        });
    }
    Ok(())
}

fn collect_homebrew(
    run: &mut Collector<'_>,
    catalogue: &Catalogue,
    transport: &dyn Transport,
) -> Result<(), TiersError> {
    let formulas = channel(catalogue, |entry| channels(entry).0);
    let mut pacer = IntervalPacer::new(HOMEBREW_INTERVAL);
    let found = batch::each(&formulas, &mut pacer, |(_, name), pacer| {
        attempt(homebrew_installs(transport, name, pacer), |error| {
            distribution_failure(error, Source::Homebrew)
        })
    })?;
    run.out.summary.fetches += formulas.len();
    for ((slug, name), result) in formulas.iter().zip(found) {
        let fetch = Fetch {
            slug,
            source: Source::Homebrew,
            of: name,
            slot: |values| &mut values.homebrew,
        };
        let result = result.map(|found| found.map(|installs| installs.on_request_365d));
        run.settle(&fetch, result, || format!("Homebrew has no formula {name}"));
    }
    Ok(())
}

fn collect_repology(
    run: &mut Collector<'_>,
    catalogue: &Catalogue,
    transport: &dyn Transport,
) -> Result<(), TiersError> {
    let projects = channel(catalogue, |entry| channels(entry).1);
    let mut pacer = IntervalPacer::new(REPOLOGY_INTERVAL);
    let found = batch::each(&projects, &mut pacer, |(_, name), pacer| {
        attempt(repology_packaging(transport, name, pacer), |error| {
            distribution_failure(error, Source::Repology)
        })
    })?;
    run.out.summary.fetches += projects.len();
    for ((slug, name), result) in projects.iter().zip(found) {
        let fetch = Fetch {
            slug,
            source: Source::Repology,
            of: name,
            slot: |values| &mut values.repology,
        };
        let result = result.map(|found| found.map(|packaging| packaging.families));
        run.settle(&fetch, result, || format!("Repology has no project {name}"));
    }
    Ok(())
}

fn collect_docker(
    run: &mut Collector<'_>,
    catalogue: &Catalogue,
    transport: &dyn Transport,
) -> Result<(), TiersError> {
    for (slug, count) in fetch_docker(catalogue, transport)? {
        run.out.summary.fetches += usize::from(count.requested);
        let fetch = Fetch {
            slug: &slug,
            source: Source::Docker,
            of: &count.of,
            slot: |values| &mut values.docker,
        };
        run.slot(&slug).notes.extend(count.notes);
        match (count.error, count.pulls) {
            (Some(error), _) => run.fell_back(&fetch, &error),
            (None, Some(pulls)) => run.found(&fetch, run.today, pulls),
            (None, None) => {}
        }
    }
    Ok(())
}

/// The GitHub repo of `entry`, or the note that explains why it has none.
pub(crate) fn github_repo(entry: &Entry) -> Result<RepoRef, String> {
    let link = entry.repo.as_str();
    let unknown = "repository signals unknown";
    let Some(normalized) = normalize_repo_url(link) else {
        return Err(format!("repo {link} names no owner and name: {unknown}"));
    };
    let Some(path) = normalized.strip_prefix("github.com/") else {
        let host = normalized.split('/').next().unwrap_or_default();
        return Err(format!("repo host {host} has no facts source: {unknown}"));
    };
    RepoRef::parse(path).map_err(|error| format!("{error}: {unknown}"))
}

/// The Homebrew formula and the Repology project of an app.
fn channels(entry: &Entry) -> (Option<&str>, Option<&str>) {
    match (&entry.kind, &entry.distribution) {
        (EntryKind::App, Some(distribution)) => (
            distribution.homebrew.as_deref(),
            distribution.repology.as_deref(),
        ),
        _ => (None, None),
    }
}

/// Every entry's name on one distribution channel, in slug order.
fn channel(catalogue: &Catalogue, pick: impl Fn(&Entry) -> Option<&str>) -> Vec<(Slug, String)> {
    catalogue
        .entries()
        .iter()
        .filter_map(|(slug, entry)| pick(entry).map(|name| (slug.clone(), name.to_owned())))
        .collect()
}

fn kind(catalogue: &Catalogue, slug: &Slug) -> Option<EntryKind> {
    catalogue.entry(slug).map(|entry| entry.kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(repo: &str, extra: &str) -> Entry {
        let text = format!(
            "name = \"T\"\nkind = \"app\"\nrepo = \"{repo}\"\ncrates = []\ntopics = [\"ml\"]\n\
             added = 2026-01-01\n{extra}\n[description]\ntext = \"t\"\nsource = \"human\"\n"
        );
        toml::from_str(&text).expect("entry parses")
    }

    #[test]
    fn github_links_become_owner_and_name() {
        let repo = github_repo(&entry("https://github.com/ML-Rust/Tool.git", "")).expect("repo");
        assert_eq!(repo.to_string(), "ml-rust/tool");
    }

    #[test]
    fn other_hosts_and_bad_links_name_the_reason() {
        let note = github_repo(&entry("https://gitlab.com/a/b", "")).expect_err("gitlab");
        assert_eq!(
            note,
            "repo host gitlab.com has no facts source: repository signals unknown"
        );
        let note = github_repo(&entry("https://github.com/a", "")).expect_err("no name");
        assert!(note.contains("names no owner and name"), "{note}");
    }

    #[test]
    fn channels_come_from_an_app_distribution() {
        let app = entry(
            "https://github.com/a/b",
            "[distribution]\nhomebrew = \"b\"\n",
        );
        assert_eq!(channels(&app), (Some("b"), None));
        assert_eq!(channels(&entry("https://github.com/a/b", "")), (None, None));
    }
}
