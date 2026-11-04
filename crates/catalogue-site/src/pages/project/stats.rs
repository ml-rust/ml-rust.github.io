//! The stat strip at the top of an entry page.

use catalogue_core::{
    COMMITS_SIGNAL, CONTRIBUTORS_SIGNAL, DOWNLOADS_SIGNAL, Entry, EntryKind, EntryReport,
    FORKS_SIGNAL, NOT_APPLICABLE, NOT_MEASURED, STARS_SIGNAL, TierReport,
};

use crate::pages::shared::{
    ago_text, date_text, dependents_text, downloads_signal, grouped, whole_text,
};

/// The signal of the latest version.
const VERSION_SIGNAL: &str = "latest_version";
/// The signal of the latest release date.
const RELEASE_SIGNAL: &str = "last_release";
/// The stats of the repository row: the label and the signal of each.
const REPOSITORY_STATS: [(&str, &str); 4] = [
    ("Stars", STARS_SIGNAL),
    ("Forks", FORKS_SIGNAL),
    ("Contributors", CONTRIBUTORS_SIGNAL),
    ("Commits", COMMITS_SIGNAL),
];

/// One stat of the strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stat {
    /// The stat name.
    pub label: &'static str,
    /// The value.
    pub value: String,
    /// Whether the value prints in the muted colour: a value not measured.
    pub quiet: bool,
    /// The date below the value, as an ISO date.
    pub date: Option<String>,
    /// The date below the value, as a person reads it.
    pub date_text: Option<String>,
}

impl Stat {
    fn plain(label: &'static str, value: String) -> Self {
        Self {
            label,
            quiet: value == NOT_MEASURED || value == NOT_APPLICABLE,
            value,
            date: None,
            date_text: None,
        }
    }
}

/// The data of the stat strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatStrip {
    /// The stats, with the check date last.
    pub stats: Vec<Stat>,
    /// The repository stats: stars, forks, contributors and commits. Empty when the check
    /// measured none of them.
    pub repository: Vec<Stat>,
    /// The check date as an ISO date.
    pub checked: String,
    /// The check date as a person reads it.
    pub checked_text: String,
}

impl StatStrip {
    /// The strip for `entry` from `record` of the check `report`.
    pub(crate) fn new(entry: &Entry, report: &TierReport, record: &EntryReport) -> Self {
        let has_crates = !entry.crates.is_empty();
        let library = entry.kind == EntryKind::Library;
        let mut stats = Vec::new();
        let value_of = |name: &str| record.value_of(name).map(str::to_owned);
        let downloads = downloads_signal(entry.kind, Some(record));
        stats.push(Stat::plain(
            if downloads == DOWNLOADS_SIGNAL {
                "Downloads (30 days)"
            } else {
                "Downloads per release"
            },
            value_of(downloads).map_or_else(|| NOT_MEASURED.to_owned(), |count| grouped(&count)),
        ));
        stats.push(Stat::plain(
            "Dependents",
            dependents_text(entry.kind, Some(record)),
        ));
        if library || record.value_of(VERSION_SIGNAL).is_some() {
            stats.push(Stat::plain(
                "Version",
                value_of(VERSION_SIGNAL).unwrap_or_else(|| NOT_MEASURED.to_owned()),
            ));
        }
        if has_crates || record.value_of(RELEASE_SIGNAL).is_some() {
            stats.push(released(record, report));
        }
        let repository = repository_stats(record);
        let checked = report.generated.to_string();
        Self {
            stats,
            repository,
            checked_text: date_text(&checked).unwrap_or_else(|| checked.clone()),
            checked,
        }
    }
}

/// The repository row, with the unmeasured values in place. Empty when none is measured.
fn repository_stats(record: &EntryReport) -> Vec<Stat> {
    let measured = REPOSITORY_STATS
        .iter()
        .any(|(_, name)| record.value_of(name).is_some());
    if !measured {
        return Vec::new();
    }
    REPOSITORY_STATS
        .iter()
        .map(|&(label, name)| Stat::plain(label, grouped(&whole_text(Some(record), name))))
        .collect()
}

/// The release stat: how long ago, and the date.
fn released(record: &EntryReport, report: &TierReport) -> Stat {
    let Some(iso) = record.value_of(RELEASE_SIGNAL) else {
        return Stat::plain("Released", NOT_MEASURED.to_owned());
    };
    let shown = date_text(iso);
    Stat {
        label: "Released",
        value: ago_text(iso, report.generated)
            .or_else(|| shown.clone())
            .unwrap_or_else(|| iso.to_owned()),
        quiet: false,
        date_text: shown,
        date: Some(iso.to_owned()),
    }
}
