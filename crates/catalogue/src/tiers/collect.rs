//! The network signals of `catalogue tiers`: repo facts and app distribution counts.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use catalogue_core::{Catalogue, Date, Entry, EntryKind, Slug, normalize_repo_url};
use catalogue_signals::distribution::{
    HOMEBREW_INTERVAL, REPOLOGY_INTERVAL, homebrew_installs, release_downloads, repology_packaging,
};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::fallback::{
    Failed, FetchFailure, FetchSummary, Source, attempt, distribution_failure, github_failure,
    link_failure, recover,
};
use catalogue_signals::github::{Contributors, RepoRef, repo_contributors, repo_facts};
use catalogue_signals::history::{Dated, NetworkValues, ReleaseValues, RepoValues};
use catalogue_signals::http::{IntervalPacer, Pacer, Transport};

use super::cadence::{Due, FetchMode, backstop_days, fallback_limit, fetches, network_sources};
use super::docker::{fetch_docker, images};
use super::error::TiersError;
use super::links::{TargetRepo, fetch_links};
use super::stored::{Memory, StoredEntry, stored_entries};
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
    /// stored date for a cached link check, a fallback or a reused value.
    pub(crate) values: NetworkValues,
    pub(crate) notes: Vec<String>,
    /// The date and subject of the README fetch that found no README.
    pub(crate) no_readme: Option<(Date, String)>,
    /// Whether this run asked any source about the entry.
    pub(crate) asked: bool,
    /// Whether the row of this run is no check: the run reused stored values and sent no request.
    pub(crate) reused: bool,
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

/// Collects fetch results. A failed fetch reads the snapshots in `memory`.
struct Collector<'a> {
    memory: Memory<'a>,
    /// The state of each entry before this run. Empty when the run reuses nothing.
    stored: BTreeMap<Slug, StoredEntry>,
    today: Date,
    out: Collected,
}

impl Collector<'_> {
    /// Whether `slug` needs a fetch of `source` on this run, by the table in
    /// [`super::cadence`]. A yes marks the entry asked and clears the reused value, so an answer
    /// of "no such name" leaves nothing behind.
    fn wants<T>(&mut self, source: Source, slug: &Slug, slot: Slot<T>) -> bool {
        let due = self.due(source, slug, slot);
        if !fetches(self.memory.fetch, source, due) {
            return false;
        }
        let fetched = self.slot(slug);
        fetched.asked = true;
        *slot(&mut fetched.values) = None;
        if source == Source::Readme {
            fetched.no_readme = None;
        }
        true
    }

    /// What the fetch decision reads about `source` of `slug`.
    fn due<T>(&mut self, source: Source, slug: &Slug, slot: Slot<T>) -> Due {
        let today = self.today;
        let pushed = self.pushed(slug);
        let Some(entry) = self.stored.get_mut(slug) else {
            return Due::default();
        };
        let mut date = slot(&mut entry.values).as_ref().map(|found| found.date);
        if source == Source::Readme {
            date = date.or(entry.no_readme.as_ref().map(|(on, _)| *on));
        }
        Due {
            new_entry: entry.new_entry,
            has_value: date.is_some(),
            past_backstop: date
                .is_some_and(|on| today.days_since(on) > backstop_days(slug.as_str())),
            pushed,
            released: entry.released,
        }
    }

    /// Whether the facts of this run show a newer push than the stored facts.
    fn pushed(&self, slug: &Slug) -> bool {
        let Some(entry) = self.stored.get(slug) else {
            return false;
        };
        let fresh = self
            .out
            .entries
            .get(slug)
            .and_then(|fetched| fetched.values.facts.as_ref())
            .and_then(|facts| facts.value.pushed_at());
        fresh > entry.pushed_at
    }

    /// The items of `items` whose slug needs a fetch of `source`.
    fn wanted<I: Clone>(
        &mut self,
        source: Source,
        slot: Slot<impl Sized>,
        items: &[(Slug, I)],
    ) -> Vec<(Slug, I)> {
        items
            .iter()
            .filter(|(slug, _)| self.wants(source, slug, slot))
            .cloned()
            .collect()
    }

    /// Settles the entries this run asked nothing about: their notes carry over, and their rows
    /// count as no check. A contributor list too large to read keeps its note when reused.
    fn finish(&mut self) {
        for (slug, fetched) in &mut self.out.entries {
            let Some(stored) = self.stored.get_mut(slug) else {
                continue;
            };
            if !fetched.asked {
                fetched.reused = !stored.checked_today;
                for note in stored.notes.drain(..).chain(stored.dropped.drain(..)) {
                    if !fetched.notes.contains(&note) {
                        fetched.notes.push(note);
                    }
                }
            }
            if let Some(found) = &fetched.values.contributors
                && found.value == Contributors::TooLarge
            {
                let note = too_large_note(&found.of);
                if !fetched.notes.contains(&note) {
                    fetched.notes.push(note);
                }
            }
        }
    }

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
            .memory
            .history
            .previous_network(fetch.slug.as_str(), today)
            .cloned()
            .unwrap_or_default();
        if let Some(record) = self.memory.readmes.records.get(fetch.slug.as_str())
            && let Some(readme) = &record.readme
        {
            earlier.readme = Some(Dated {
                date: record.date,
                of: record.of.clone(),
                value: readme.clone(),
            });
        }
        let earlier = (fetch.slot)(&mut earlier).take();
        let found = recover(
            fetch.source,
            earlier,
            fetch.of,
            error,
            today,
            fallback_limit(fetch.source, fetch.slug.as_str(), self.memory.max_age_days),
        );
        self.out.summary.failures.push(FetchFailure {
            slug: fetch.slug.clone(),
            source: fetch.source,
            from: found.value.as_ref().map(|value| value.date),
            error: error.to_owned(),
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
/// Every entry with a GitHub repo gets its repo facts, then its contributor count unless the
/// facts find the repo gone. Every app with a GitHub
/// repo gets its release downloads, then the Homebrew and Repology counts its
/// `distribution` names, then the Docker Hub pulls of its listed images. Every
/// entry with crates gets its repo link check, which `snapshot` supplies the release for.
/// Every entry on GitHub, Codeberg or GitLab gets its README, unless the facts find the repo gone.
/// The `github` pacer spaces every GitHub phase, so a spent budget carries from one phase to the
/// next. Every other source has its own pacer.
///
/// [`catalogue_signals::fallback`] sorts every error. A fatal error ends the run.
/// A definite "no such name" becomes a note. Any other error reuses that source's
/// value from the newest snapshot in `history` that holds the entry, up to
/// `max_age_days` old, and otherwise leaves the signal unknown. Only these
/// network fetches fall back: the dump and the RustSec archive have their own files.
///
/// `memory.fetch` picks what goes out, from the table in [`super::cadence`]. A value this run does
/// not fetch stays as stored, with its true date. It adds no fetch and no failure to the summary.
pub(crate) fn fetch_all(
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    network: Network<'_>,
    memory: Memory<'_>,
    github: &mut dyn Pacer,
) -> Result<Collected, TiersError> {
    let stored = stored_entries(catalogue, snapshot, memory);
    let mut run = Collector {
        memory,
        today: memory.today,
        out: Collected {
            entries: catalogue
                .entries()
                .keys()
                .map(|slug| {
                    let fetched = stored
                        .get(slug)
                        .map_or_else(Fetched::default, |entry| Fetched {
                            values: entry.values.clone(),
                            no_readme: entry.no_readme.clone(),
                            ..Fetched::default()
                        });
                    (slug.clone(), fetched)
                })
                .collect(),
            summary: FetchSummary::default(),
        },
        stored,
    };
    let online = match network {
        Network::Online {
            transport,
            token,
            cache_dir,
        } => Some((transport, token, cache_dir)),
        Network::Offline if memory.fetch == FetchMode::All => return Ok(run.out),
        Network::Offline => None,
    };
    let mut repos: Vec<(Slug, RepoRef)> = Vec::new();
    for (slug, entry) in catalogue.entries() {
        match github_repo(entry) {
            Ok(repo) => repos.push((slug.clone(), repo)),
            Err(note) => run.note(slug, note),
        }
    }
    if let Some((transport, token, cache_dir)) = online {
        for source in network_sources() {
            match source {
                Source::RepoFacts => collect_facts(&mut run, &repos, transport, token, github)?,
                Source::RepoLink => collect_links(
                    &mut run, catalogue, snapshot, transport, token, cache_dir, github,
                )?,
                Source::Contributors => {
                    collect_contributors(&mut run, &repos, transport, token, github)?;
                }
                Source::Readme => collect_readmes(&mut run, catalogue, transport, token, github)?,
                Source::Releases => {
                    let apps: Vec<(Slug, RepoRef)> = repos
                        .iter()
                        .filter(|(slug, _)| kind(catalogue, slug) == Some(EntryKind::App))
                        .cloned()
                        .collect();
                    collect_releases(&mut run, &apps, transport, token, github)?;
                }
                Source::Homebrew => collect_homebrew(&mut run, catalogue, transport)?,
                Source::Repology => collect_repology(&mut run, catalogue, transport)?,
                Source::Docker => collect_docker(&mut run, catalogue, transport)?,
            }
        }
    }
    run.finish();
    run.out.summary.failures.sort();
    Ok(run.out)
}

fn collect_facts(
    run: &mut Collector<'_>,
    repos: &[(Slug, RepoRef)],
    transport: &dyn Transport,
    token: &str,
    github: &mut dyn Pacer,
) -> Result<(), TiersError> {
    let today = run.today.civil();
    let repos = run.wanted(Source::RepoFacts, |values| &mut values.facts, repos);
    let found = batch::each(&repos, github, |(_, repo), pacer| {
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
    github: &mut dyn Pacer,
) -> Result<(), TiersError> {
    let entries: Vec<(&Slug, &Entry)> = catalogue
        .entries()
        .iter()
        .filter(|(slug, entry)| {
            !entry.crates.is_empty() && run.wants(Source::RepoLink, slug, |values| &mut values.link)
        })
        .collect();
    let links = fetch_links(
        &entries, snapshot, transport, token, cache_dir, run.today, github,
    )?;
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

/// Counts the contributors of every repo the facts did not find gone: one request per repo.
///
/// A stored count of a gone repo goes too, as a fetch would not ask for it.
///
/// A list GitHub calls too large is an answer, so it adds a note and never falls back.
fn collect_contributors(
    run: &mut Collector<'_>,
    repos: &[(Slug, RepoRef)],
    transport: &dyn Transport,
    token: &str,
    github: &mut dyn Pacer,
) -> Result<(), TiersError> {
    let mut live: Vec<(Slug, RepoRef)> = Vec::new();
    for (slug, repo) in repos {
        if is_gone(run, slug) {
            run.slot(slug).values.contributors = None;
        } else {
            live.push((slug.clone(), repo.clone()));
        }
    }
    let live = run.wanted(
        Source::Contributors,
        |values| &mut values.contributors,
        &live,
    );
    let found = batch::each(&live, github, |(_, repo), pacer| {
        attempt(
            repo_contributors(transport, token, repo, pacer),
            github_failure,
        )
    })?;
    run.out.summary.fetches += live.len();
    for ((slug, repo), result) in live.iter().zip(found) {
        let of = repo.to_string();
        let fetch = Fetch {
            slug,
            source: Source::Contributors,
            of: &of,
            slot: |values| &mut values.contributors,
        };
        if matches!(result, Ok(Contributors::TooLarge)) {
            run.note(slug, too_large_note(&of));
        }
        run.settle(&fetch, result.map(Some), String::new);
    }
    Ok(())
}

/// The note on a contributor list GitHub will not serve.
fn too_large_note(repo: &str) -> String {
    format!("GitHub calls the contributor list of {repo} too large to read: not measured")
}

/// Fetches the README of every entry on a host that serves one.
///
/// A repo with no README adds no note: the entry page leaves the section out. The run records the
/// empty answer, so the next run does not ask again without a reason. A failed fetch reads the
/// README store, like every other source reads its snapshot.
fn collect_readmes(
    run: &mut Collector<'_>,
    catalogue: &Catalogue,
    transport: &dyn Transport,
    token: &str,
    github: &mut dyn Pacer,
) -> Result<(), TiersError> {
    let mut repos: Vec<(Slug, TargetRepo)> = Vec::new();
    for (slug, entry) in catalogue.entries() {
        if is_gone(run, slug) {
            let fetched = run.slot(slug);
            fetched.values.readme = None;
            fetched.no_readme = None;
        } else if let Some(repo) = TargetRepo::of(entry)
            && run.wants(Source::Readme, slug, |values| &mut values.readme)
        {
            repos.push((slug.clone(), repo));
        }
    }
    let found = batch::each(&repos, github, |(_, repo), pacer| {
        attempt(repo.readme(transport, token, pacer), link_failure)
    })?;
    run.out.summary.fetches += repos.len();
    for ((slug, repo), result) in repos.iter().zip(found) {
        let of = repo.to_string();
        let fetch = Fetch {
            slug,
            source: Source::Readme,
            of: &of,
            slot: |values| &mut values.readme,
        };
        match result {
            Ok(None) | Err(Failed::Definitive(_)) => {
                let today = run.today;
                run.slot(slug).no_readme = Some((today, of.clone()));
            }
            other => run.settle(&fetch, other, String::new),
        }
    }
    Ok(())
}

/// Whether this run found the repo of `slug` gone.
fn is_gone(run: &Collector<'_>, slug: &Slug) -> bool {
    run.out
        .entries
        .get(slug)
        .and_then(|fetched| fetched.values.facts.as_ref())
        .is_some_and(|facts| facts.value == RepoValues::Gone)
}

fn collect_releases(
    run: &mut Collector<'_>,
    apps: &[(Slug, RepoRef)],
    transport: &dyn Transport,
    token: &str,
    github: &mut dyn Pacer,
) -> Result<(), TiersError> {
    let apps = run.wanted(Source::Releases, |values| &mut values.releases, apps);
    let found = batch::each(&apps, github, |(_, repo), pacer| {
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
    let formulas = run.wanted(Source::Homebrew, |values| &mut values.homebrew, &formulas);
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
    let projects = run.wanted(Source::Repology, |values| &mut values.repology, &projects);
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
    let wanted: BTreeSet<Slug> = catalogue
        .entries()
        .iter()
        .filter(|(slug, entry)| {
            !images(entry).is_empty()
                && run.wants(Source::Docker, slug, |values| &mut values.docker)
        })
        .map(|(slug, _)| slug.clone())
        .collect();
    for (slug, count) in fetch_docker(catalogue, &wanted, transport)? {
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
pub(super) fn channels(entry: &Entry) -> (Option<&str>, Option<&str>) {
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
