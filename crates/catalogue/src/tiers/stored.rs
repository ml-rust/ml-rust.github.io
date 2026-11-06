//! What the history holds for each entry, sorted out for a run that can reuse it.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, Date, Entry, EntryKind, Slug};
use catalogue_signals::assemble::{CrateIndex, primary_release};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::history::{Dated, History, NetworkValues, ReadmeRecord, ReadmeStore};
use jiff::Timestamp;

use super::cadence::FetchMode;
use super::collect::{Fetched, channels, github_repo};
use super::docker::images;
use super::links::{TargetRepo, subject};

/// What a run reads about earlier runs.
#[derive(Clone, Copy)]
pub(super) struct Memory<'a> {
    /// The snapshots.
    pub(super) history: &'a History,
    /// The newest README of each entry.
    pub(super) readmes: &'a ReadmeStore,
    /// The age limit of `fallback.max_age_days`.
    pub(super) max_age_days: u32,
    /// What this run may fetch.
    pub(super) fetch: FetchMode,
    /// The run date.
    pub(super) today: Date,
}

/// The stored state of one entry.
#[derive(Debug, Default)]
pub(super) struct StoredEntry {
    /// The stored values that still describe the entry, each with its true date.
    pub(super) values: NetworkValues,
    /// The date and subject of a stored README fetch that found no README.
    pub(super) no_readme: Option<(Date, String)>,
    /// The last push the stored facts hold.
    pub(super) pushed_at: Option<Timestamp>,
    /// Whether the entry has no stored network values.
    pub(super) new_entry: bool,
    /// Whether the dump shows a newer release than the stored link check read.
    pub(super) released: bool,
    /// Whether a snapshot of the run date already checked the entry. A later run on that day
    /// replaces the file, so its row must stay a check.
    pub(super) checked_today: bool,
    /// The fetch notes of the stored row.
    pub(super) notes: Vec<String>,
    /// One note per stored value that describes another subject.
    pub(super) dropped: Vec<String>,
}

/// The subject each source of an entry asks about today. `None` for a source it does not use.
struct Subjects {
    facts: Option<String>,
    releases: Option<String>,
    homebrew: Option<String>,
    repology: Option<String>,
    docker: Option<String>,
    readme: Option<String>,
    link: LinkSubject,
}

/// What a stored link check must describe.
enum LinkSubject {
    /// The entry asks for no link check.
    Nothing,
    /// The entry has crates on a host no check can read.
    UnsupportedHost(String),
    /// The primary crate and repo. A check of an older version still describes them.
    Release {
        name: String,
        repo: String,
        current: String,
    },
}

impl LinkSubject {
    /// Whether a value about `of` describes this subject.
    fn matches(&self, of: &str) -> bool {
        match self {
            Self::Nothing => false,
            Self::UnsupportedHost(repo) => of == repo,
            Self::Release { name, repo, .. } => {
                of.starts_with(&format!("{name} ")) && of.ends_with(&format!(" in {repo}"))
            }
        }
    }

    /// Whether a value about `of` read an older release than the dump shows.
    fn is_older(&self, of: &str) -> bool {
        matches!(self, Self::Release { current, .. } if of != current)
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Nothing => None,
            Self::UnsupportedHost(text) => Some(text),
            Self::Release { current, .. } => Some(current),
        }
    }
}

fn subjects(entry: &Entry, snapshot: &DumpSnapshot, index: &CrateIndex) -> Subjects {
    let repo = github_repo(entry).ok().map(|repo| repo.to_string());
    let target = TargetRepo::of(entry);
    let is_app = entry.kind == EntryKind::App;
    let (homebrew, repology) = channels(entry);
    let link = if entry.crates.is_empty() {
        LinkSubject::Nothing
    } else {
        match (&target, primary_release(entry, snapshot, index)) {
            (None, _) => LinkSubject::UnsupportedHost(entry.repo.to_string()),
            (Some(_), None) => LinkSubject::Nothing,
            (Some(found), Some(release)) => LinkSubject::Release {
                name: release.name.clone(),
                repo: found.to_string(),
                current: subject(found, &release),
            },
        }
    };
    let listed = images(entry);
    Subjects {
        releases: repo.clone().filter(|_| is_app),
        facts: repo,
        homebrew: homebrew.map(str::to_owned),
        repology: repology.map(str::to_owned),
        docker: (!listed.is_empty()).then(|| listed.join(", ")),
        readme: target.map(|found| found.to_string()),
        link,
    }
}

/// Keeps `value` when it describes `expected`, and adds a note when it describes something else.
fn checked<T>(
    value: &mut Option<Dated<T>>,
    expected: Option<&str>,
    what: &str,
    dropped: &mut Vec<String>,
) {
    let Some(found) = value else {
        return;
    };
    if expected == Some(found.of.as_str()) {
        return;
    }
    if let Some(expected) = expected {
        dropped.push(drop_note(what, &found.of, expected));
    }
    *value = None;
}

fn drop_note(what: &str, of: &str, expected: &str) -> String {
    format!("the stored {what} describes {of}, not {expected}: signal unknown until the next fetch")
}

/// The stored README record of `record`, when it describes `expected`.
fn read_record(entry: &mut StoredEntry, record: &ReadmeRecord, expected: Option<&str>) {
    if expected != Some(record.of.as_str()) {
        return;
    }
    match &record.readme {
        Some(readme) => {
            entry.values.readme = Some(Dated {
                date: record.date,
                of: record.of.clone(),
                value: readme.clone(),
            });
        }
        None => entry.no_readme = Some((record.date, record.of.clone())),
    }
}

/// The stored state of every entry. Empty for [`FetchMode::All`], which reuses nothing.
pub(super) fn stored_entries(
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    memory: Memory<'_>,
) -> BTreeMap<Slug, StoredEntry> {
    if memory.fetch == FetchMode::All {
        return BTreeMap::new();
    }
    let index = CrateIndex::new(snapshot);
    catalogue
        .entries()
        .iter()
        .map(|(slug, entry)| {
            let wanted = subjects(entry, snapshot, &index);
            (slug.clone(), stored_entry(slug, wanted, memory))
        })
        .collect()
}

fn stored_entry(slug: &Slug, wanted: Subjects, memory: Memory<'_>) -> StoredEntry {
    let mut entry = StoredEntry::default();
    if let Some((date, row)) = memory.history.stored_row(slug.as_str(), memory.today) {
        entry.checked_today = date == memory.today && !row.reused;
        entry.notes = row.fetch_notes.clone();
        entry.values = row.network.clone().unwrap_or_default();
    }
    let values = &mut entry.values;
    let dropped = &mut entry.dropped;
    let facts = wanted.facts.as_deref();
    checked(&mut values.facts, facts, "repo facts", dropped);
    checked(
        &mut values.contributors,
        facts,
        "contributor count",
        dropped,
    );
    let releases = wanted.releases.as_deref();
    checked(&mut values.releases, releases, "release count", dropped);
    let homebrew = wanted.homebrew.as_deref();
    checked(&mut values.homebrew, homebrew, "Homebrew count", dropped);
    let repology = wanted.repology.as_deref();
    checked(&mut values.repology, repology, "Repology count", dropped);
    let docker = wanted.docker.as_deref();
    checked(&mut values.docker, docker, "Docker Hub count", dropped);
    if let Some(found) = &values.link
        && !wanted.link.matches(&found.of)
    {
        if let Some(expected) = wanted.link.text() {
            dropped.push(drop_note("repo link check", &found.of, expected));
        }
        values.link = None;
    }
    entry.released = values
        .link
        .as_ref()
        .is_some_and(|found| wanted.link.is_older(&found.of));
    entry.pushed_at = values.facts.as_ref().and_then(|f| f.value.pushed_at());
    if let Some(record) = memory.readmes.records.get(slug.as_str()) {
        read_record(&mut entry, record, wanted.readme.as_deref());
    }
    entry.new_entry = if wanted.facts.is_some() {
        entry.values.facts.is_none()
    } else {
        entry.values.is_empty() && entry.no_readme.is_none()
    };
    entry
}

/// The README records of a finished run, for the next run to reuse.
pub(super) fn readme_store(entries: &BTreeMap<Slug, Fetched>) -> ReadmeStore {
    let records = entries
        .iter()
        .filter_map(|(slug, fetched)| {
            let record = match (&fetched.values.readme, &fetched.no_readme) {
                (Some(found), _) => ReadmeRecord {
                    date: found.date,
                    of: found.of.clone(),
                    readme: Some(found.value.clone()),
                },
                (None, Some((date, of))) => ReadmeRecord {
                    date: *date,
                    of: of.clone(),
                    readme: None,
                },
                (None, None) => return None,
            };
            Some((slug.to_string(), record))
        })
        .collect();
    ReadmeStore { records }
}
