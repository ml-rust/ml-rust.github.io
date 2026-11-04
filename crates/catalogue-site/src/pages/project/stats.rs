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
/// What the Downloads value counts when it reads the 30-day window.
const DOWNLOADS_HINT: &str = "Downloads over 30 days";
/// What the Downloads value counts when it reads release assets.
const PER_RELEASE_HINT: &str = "Binary downloads per release";
/// The repository stats: the label and the signal of each.
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
    /// What the stat counts, when the name alone does not say.
    pub hint: Option<&'static str>,
    /// The value. Empty when only the date is known.
    pub value: String,
    /// Whether the value prints in the muted colour: a value not measured.
    pub quiet: bool,
    /// The date after the value.
    pub date: Option<StatDate>,
}

/// The date of a stat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatDate {
    /// The ISO date.
    pub iso: String,
    /// How long ago, as the page prints it.
    pub text: String,
    /// The exact date as a person reads it.
    pub exact: String,
}

impl Stat {
    fn plain(label: &'static str, value: String) -> Self {
        Self {
            label,
            hint: None,
            quiet: value == NOT_MEASURED || value == NOT_APPLICABLE,
            value,
            date: None,
        }
    }
}

/// The data of the stat strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatStrip {
    /// Downloads, dependents, stars, forks, contributors, commits and the latest release.
    pub stats: Vec<Stat>,
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
        stats.push(Stat {
            hint: Some(if downloads == DOWNLOADS_SIGNAL {
                DOWNLOADS_HINT
            } else {
                PER_RELEASE_HINT
            }),
            ..Stat::plain(
                "Downloads",
                value_of(downloads)
                    .map_or_else(|| NOT_MEASURED.to_owned(), |count| grouped(&count)),
            )
        });
        stats.push(Stat::plain(
            "Dependents",
            dependents_text(entry.kind, Some(record)),
        ));
        stats.extend(repository_stats(record));
        if library
            || has_crates
            || record.value_of(VERSION_SIGNAL).is_some()
            || record.value_of(RELEASE_SIGNAL).is_some()
        {
            stats.push(latest_release(record, report));
        }
        let checked = report.generated.to_string();
        Self {
            stats,
            checked_text: date_text(&checked).unwrap_or_else(|| checked.clone()),
            checked,
        }
    }
}

/// The repository stats, with the unmeasured values in place. Empty when none is measured.
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

/// The release stat: the version and how long ago, with the exact date in the title.
fn latest_release(record: &EntryReport, report: &TierReport) -> Stat {
    let date = record.value_of(RELEASE_SIGNAL).map(|iso| {
        let exact = date_text(iso).unwrap_or_else(|| iso.to_owned());
        StatDate {
            iso: iso.to_owned(),
            text: ago_text(iso, report.generated).unwrap_or_else(|| exact.clone()),
            exact,
        }
    });
    let value = match (record.value_of(VERSION_SIGNAL), &date) {
        (Some(version), _) => version.to_owned(),
        (None, Some(_)) => String::new(),
        (None, None) => NOT_MEASURED.to_owned(),
    };
    Stat {
        date,
        ..Stat::plain("Latest release", value)
    }
}
