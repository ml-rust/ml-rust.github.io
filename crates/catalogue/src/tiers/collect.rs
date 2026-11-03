//! The network signals of `catalogue tiers`: repo facts and app distribution counts.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, Entry, EntryKind, Slug, normalize_repo_url};
use catalogue_signals::distribution::{
    HOMEBREW_INTERVAL, HomebrewInstalls, RELEASES_INTERVAL, REPOLOGY_INTERVAL, ReleaseDownloads,
    RepologyPackaging, homebrew_installs, release_downloads, repology_packaging,
};
use catalogue_signals::github::{GITHUB_INTERVAL, GithubError, RepoFacts, RepoRef, repo_facts};
use catalogue_signals::http::{IntervalPacer, Transport};
use jiff::civil;

use super::command::TiersError;
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
    },
}

/// The network signals of one entry, with a note for each source that had no answer.
#[derive(Debug, Default)]
pub(crate) struct Fetched {
    pub(crate) facts: Option<RepoFacts>,
    pub(crate) releases: Option<ReleaseDownloads>,
    pub(crate) homebrew: Option<HomebrewInstalls>,
    pub(crate) repology: Option<RepologyPackaging>,
    pub(crate) notes: Vec<String>,
}

/// Fetches the network signals of every entry, keyed by slug.
///
/// Every entry with a GitHub repo gets its repo facts. Every app with a GitHub
/// repo gets its release downloads, then the Homebrew and Repology counts its
/// `distribution` names. Each source has its own pacer. A GraphQL error for one
/// repo becomes a note. Any other error ends the run.
pub(crate) fn fetch_all(
    catalogue: &Catalogue,
    network: Network<'_>,
    today: civil::Date,
) -> Result<BTreeMap<Slug, Fetched>, TiersError> {
    let mut out: BTreeMap<Slug, Fetched> = catalogue
        .entries()
        .keys()
        .map(|slug| (slug.clone(), Fetched::default()))
        .collect();
    let Network::Online { transport, token } = network else {
        return Ok(out);
    };
    let mut repos: Vec<(Slug, RepoRef)> = Vec::new();
    for (slug, entry) in catalogue.entries() {
        match github_repo(entry) {
            Ok(repo) => repos.push((slug.clone(), repo)),
            Err(note) => note_on(&mut out, slug, note),
        }
    }
    let mut pacer = IntervalPacer::new(GITHUB_INTERVAL);
    let facts = batch::each(&repos, &mut pacer, |(_, repo), pacer| {
        match repo_facts(transport, token, repo, today, pacer) {
            Ok(found) => Ok(Ok(found)),
            Err(error @ GithubError::Graphql { .. }) => Ok(Err(error.to_string())),
            Err(error) => Err(error),
        }
    })?;
    for ((slug, _), found) in repos.iter().zip(facts) {
        match found {
            Ok(found) => slot(&mut out, slug).facts = Some(found),
            Err(note) => note_on(&mut out, slug, note),
        }
    }
    let apps: Vec<(Slug, RepoRef)> = repos
        .into_iter()
        .filter(|(slug, _)| kind(catalogue, slug) == Some(EntryKind::App))
        .collect();
    let mut pacer = IntervalPacer::new(RELEASES_INTERVAL);
    let releases = batch::each(&apps, &mut pacer, |(_, repo), pacer| {
        release_downloads(transport, token, repo, pacer)
    })?;
    for ((slug, repo), found) in apps.iter().zip(releases) {
        match found {
            Some(found) => slot(&mut out, slug).releases = Some(found),
            None => note_on(&mut out, slug, format!("GitHub has no releases for {repo}")),
        }
    }
    let formulas = channel(catalogue, |entry| channels(entry).0);
    let mut pacer = IntervalPacer::new(HOMEBREW_INTERVAL);
    let installs = batch::each(&formulas, &mut pacer, |(_, name), pacer| {
        homebrew_installs(transport, name, pacer)
    })?;
    for ((slug, name), found) in formulas.iter().zip(installs) {
        match found {
            Some(found) => slot(&mut out, slug).homebrew = Some(found),
            None => note_on(&mut out, slug, format!("Homebrew has no formula {name}")),
        }
    }
    let projects = channel(catalogue, |entry| channels(entry).1);
    let mut pacer = IntervalPacer::new(REPOLOGY_INTERVAL);
    let packaging = batch::each(&projects, &mut pacer, |(_, name), pacer| {
        repology_packaging(transport, name, pacer)
    })?;
    for ((slug, name), found) in projects.iter().zip(packaging) {
        match found {
            Some(found) => slot(&mut out, slug).repology = Some(found),
            None => note_on(&mut out, slug, format!("Repology has no project {name}")),
        }
    }
    Ok(out)
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

/// The record of `slug`, which [`fetch_all`] creates for every entry first.
fn slot<'a>(out: &'a mut BTreeMap<Slug, Fetched>, slug: &Slug) -> &'a mut Fetched {
    out.entry(slug.clone()).or_default()
}

fn note_on(out: &mut BTreeMap<Slug, Fetched>, slug: &Slug, note: String) {
    slot(out, slug).notes.push(note);
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
