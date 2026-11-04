//! The stat strip at the top of an entry page.

use catalogue_core::{
    ACTIVE_USERS_SIGNAL, CONTRIBUTORS_SIGNAL, Entry, EntryReport, NOT_MEASURED, TierReport,
};

use super::sources::{SourceNote, distinct_sources, fact_named};
use crate::pages::shared::{ago_text, date_text, grouped, whole_text};

/// The signal of the latest version.
const VERSION_SIGNAL: &str = "latest_version";
/// The signal of the latest release date.
const RELEASE_SIGNAL: &str = "last_release";
/// The signal of the binary downloads of an app.
const BINARY_SIGNAL: &str = "binary_downloads_last3";

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
            quiet: value == NOT_MEASURED,
            value,
            date: None,
            date_text: None,
        }
    }
}

/// The data of the stat strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatStrip<'a> {
    /// The stats, with the check date last.
    pub stats: Vec<Stat>,
    /// The check date as an ISO date.
    pub checked: String,
    /// The check date as a person reads it.
    pub checked_text: String,
    /// The sources of the stats.
    pub sources: Vec<SourceNote<'a>>,
}

impl<'a> StatStrip<'a> {
    /// The strip for `entry` from `record` of the check `report`.
    pub(crate) fn new(entry: &Entry, report: &TierReport, record: &'a EntryReport) -> Self {
        let has_crates = !entry.crates.is_empty();
        let mut stats = Vec::new();
        let mut names = Vec::new();
        let value_of = |name: &str| record.value_of(name).map(str::to_owned);
        if has_crates {
            stats.push(Stat::plain(
                "Active users",
                whole_text(Some(record), ACTIVE_USERS_SIGNAL),
            ));
            names.push(ACTIVE_USERS_SIGNAL);
            stats.push(Stat::plain(
                "Version",
                value_of(VERSION_SIGNAL).unwrap_or_else(|| NOT_MEASURED.to_owned()),
            ));
            names.push(VERSION_SIGNAL);
        } else {
            if let Some(downloads) = value_of(BINARY_SIGNAL) {
                stats.push(Stat::plain(
                    "Binary downloads, last 3 releases",
                    grouped(&downloads),
                ));
                names.push(BINARY_SIGNAL);
            }
            if record.value_of(CONTRIBUTORS_SIGNAL).is_some() {
                stats.push(Stat::plain(
                    "Contributors",
                    whole_text(Some(record), CONTRIBUTORS_SIGNAL),
                ));
                names.push(CONTRIBUTORS_SIGNAL);
            }
        }
        if has_crates || record.value_of(RELEASE_SIGNAL).is_some() {
            stats.push(released(record, report));
            names.push(RELEASE_SIGNAL);
        }
        let checked = report.generated.to_string();
        Self {
            stats,
            checked_text: date_text(&checked).unwrap_or_else(|| checked.clone()),
            checked,
            sources: distinct_sources(names.iter().filter_map(|name| fact_named(record, name))),
        }
    }
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
