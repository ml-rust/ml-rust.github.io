//! Why an entry has its tier, and every signal behind it.

use catalogue_core::{EntryKind, EntryReport, STARS_SIGNAL, SignalFact, Thresholds, TierReport};

use super::measurements::{SignalGroup, signal_groups};
use super::why::Why;
use crate::href::SafeHref;
use crate::pages::shared::{NamedLink, date_text};

/// The counts-toward-tier text of the star count.
const STARS_NEVER_COUNT: &str = "No — stars never count toward a tier";

/// The data the tier and signal sections of `project.html` read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TierSection<'a> {
    /// The run date of the check.
    pub checked: String,
    /// The run date of the check as a person reads it.
    pub checked_text: String,
    /// The block that says why the entry has its tier, in plain sentences.
    pub why: Why<'a>,
    /// The thresholds version the check applied.
    pub version: u32,
    /// Every rule outcome, in rule order.
    pub reasons: Vec<ReasonLine<'a>>,
    /// The signals a rule needed and did not have, by label.
    pub unknown: Vec<&'a str>,
    /// One line per input the check left unresolved.
    pub notes: &'a [String],
    /// Every signal, in one group each.
    pub groups: Vec<SignalGroup<'a>>,
}

/// One rule outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReasonLine<'a> {
    /// The rule name.
    pub rule: &'static str,
    /// The outcome and the values compared.
    pub detail: &'a str,
}

/// One row of the signal table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SignalRow<'a> {
    /// The signal name shown to readers.
    pub label: &'a str,
    /// The raw value. `None` when the check did not measure it.
    pub value: Option<&'a str>,
    /// The source name and link. `None` without a value.
    pub source: Option<NamedLink<'a>>,
    /// The date of the data behind the value. `None` without a value.
    pub fetched: Option<String>,
    /// That date as a person reads it. `None` without a value.
    pub fetched_text: Option<String>,
    /// Whether a tier rule reads the value, as text.
    pub counts: &'static str,
}

impl<'a> TierSection<'a> {
    /// The section for `record`, from the check of `report`.
    pub(crate) fn new(
        report: &TierReport,
        record: &'a EntryReport,
        kind: EntryKind,
        thresholds: Option<&Thresholds>,
    ) -> Self {
        let checked = report.generated.to_string();
        Self {
            checked_text: date_text(&checked).unwrap_or_else(|| checked.clone()),
            checked,
            why: Why::new(record, kind, thresholds),
            version: report.thresholds_version,
            reasons: record
                .reasons
                .iter()
                .map(|reason| ReasonLine {
                    rule: reason.rule.as_str(),
                    detail: &reason.detail,
                })
                .collect(),
            unknown: record
                .unknown
                .iter()
                .map(|name| label_of(&record.signals, name))
                .collect(),
            notes: &record.notes,
            groups: signal_groups(report, record),
        }
    }
}

impl<'a> SignalRow<'a> {
    /// The row of `fact`. A fact without a value shows no source and no date.
    pub(super) fn of(fact: &'a SignalFact) -> Self {
        let measured = fact.value.is_some();
        let source = fact.source.as_ref().filter(|_| measured);
        let counts = if fact.counts_toward_tier {
            "Yes"
        } else if fact.name == STARS_SIGNAL {
            STARS_NEVER_COUNT
        } else {
            "No"
        };
        let fetched = fact
            .fetched
            .filter(|_| measured)
            .map(|date| date.to_string());
        Self {
            label: fact.shown_label(),
            value: fact.value.as_deref(),
            source: source.map(|source| NamedLink {
                name: &source.label,
                href: SafeHref::from(&source.url),
            }),
            fetched_text: fetched.as_deref().and_then(date_text),
            fetched,
            counts,
        }
    }
}

/// The label of the signal `name` in `signals`, or `name` when no fact carries it.
fn label_of<'a>(signals: &'a [SignalFact], name: &'a str) -> &'a str {
    signals
        .iter()
        .find(|fact| fact.name == name)
        .map_or(name, SignalFact::shown_label)
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, SourceRef};
    use url::Url;

    use super::*;

    fn fact(name: &str, value: Option<&str>, counts: bool) -> SignalFact {
        SignalFact {
            name: name.to_owned(),
            label: name.to_uppercase(),
            value: value.map(str::to_owned),
            source: Some(SourceRef::new(
                "dump",
                Url::parse("https://crates.io/data-access").expect("url"),
            )),
            fetched: Some(Date::new(2026, 11, 2).expect("date")),
            counts_toward_tier: counts,
        }
    }

    #[test]
    fn stars_say_they_never_count() {
        let stars = fact(STARS_SIGNAL, Some("321"), false);
        assert_eq!(SignalRow::of(&stars).counts, STARS_NEVER_COUNT);
        let other = fact("docs_present", Some("yes"), false);
        assert_eq!(SignalRow::of(&other).counts, "No");
        let counted = fact("last_release", Some("2026-09-01"), true);
        let row = SignalRow::of(&counted);
        assert_eq!(row.counts, "Yes");
        assert_eq!(row.fetched.as_deref(), Some("2026-11-02"));
        let source = row.source.expect("source");
        assert_eq!(source.name, "dump");
        assert_eq!(source.href.as_str(), "https://crates.io/data-access");
    }

    #[test]
    fn an_unmeasured_signal_shows_no_source_and_no_date() {
        let missing = fact("active_users", None, true);
        let row = SignalRow::of(&missing);
        assert_eq!(row.value, None);
        assert_eq!(row.source, None);
        assert_eq!(row.fetched, None);
    }

    #[test]
    fn unknown_signals_show_their_labels() {
        let signals = [fact("active_users", None, true)];
        assert_eq!(label_of(&signals, "active_users"), "Active dependents");
        assert_eq!(label_of(&signals, "repo_gone"), "repo_gone");
    }
}
