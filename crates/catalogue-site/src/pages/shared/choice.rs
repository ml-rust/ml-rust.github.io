//! The facts a person reads to choose a project, as text for rows and the at-a-glance list.

use catalogue_core::amount_text::whole;
use catalogue_core::{
    ACTIVE_USERS_SIGNAL, DEPENDENTS_SIGNAL, EntryReport, LICENSE_SIGNAL, NOT_MEASURED,
    RUNS_ON_SIGNAL,
};

/// The value of the fact `name` in `record`, or [`NOT_MEASURED`].
pub(crate) fn fact_text(record: Option<&EntryReport>, name: &str) -> String {
    record
        .and_then(|record| record.value_of(name))
        .unwrap_or(NOT_MEASURED)
        .to_owned()
}

/// The amount of the fact `name` in `record` as a whole number, or [`NOT_MEASURED`].
///
/// The signal table keeps the precise value. A list a person scans reads `38`, not `38.36`.
pub(crate) fn whole_text(record: Option<&EntryReport>, name: &str) -> String {
    record
        .and_then(|record| record.value_of(name))
        .and_then(whole)
        .unwrap_or_else(|| NOT_MEASURED.to_owned())
}

/// The facts one entry row shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChoiceText {
    /// The license.
    pub license: String,
    /// The backends.
    pub runs_on: String,
    /// The active users, as a whole number.
    pub active_users: String,
    /// The dependents, as a whole number.
    pub dependents: String,
}

impl ChoiceText {
    /// The facts of `record`. Without a record, every value is [`NOT_MEASURED`].
    pub(crate) fn of(record: Option<&EntryReport>) -> Self {
        Self {
            license: fact_text(record, LICENSE_SIGNAL),
            runs_on: fact_text(record, RUNS_ON_SIGNAL),
            active_users: whole_text(record, ACTIVE_USERS_SIGNAL),
            dependents: whole_text(record, DEPENDENTS_SIGNAL),
        }
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, SignalFact, SourceRef, Tier};
    use url::Url;

    use super::*;

    fn fact(name: &str, value: Option<&str>) -> SignalFact {
        SignalFact {
            name: name.to_owned(),
            label: name.to_owned(),
            value: value.map(str::to_owned),
            source: Some(SourceRef::new(
                "dump",
                Url::parse("https://crates.io/data-access").expect("url"),
            )),
            fetched: Date::new(2026, 11, 2).ok(),
            counts_toward_tier: false,
        }
    }

    fn record(signals: Vec<SignalFact>) -> EntryReport {
        EntryReport {
            tier: Tier::Listed,
            stable: false,
            dormant: None,
            demotion_pending: None,
            reasons: Vec::new(),
            unknown: Vec::new(),
            notes: Vec::new(),
            signals,
        }
    }

    #[test]
    fn a_row_prints_amounts_as_whole_numbers() {
        let found = record(vec![
            fact(LICENSE_SIGNAL, Some("MIT")),
            fact(RUNS_ON_SIGNAL, Some("CUDA, Metal")),
            fact(ACTIVE_USERS_SIGNAL, Some("38.36")),
            fact(DEPENDENTS_SIGNAL, Some("12")),
        ]);
        let text = ChoiceText::of(Some(&found));
        assert_eq!(text.license, "MIT");
        assert_eq!(text.runs_on, "CUDA, Metal");
        assert_eq!(text.active_users, "38");
        assert_eq!(text.dependents, "12");
    }

    #[test]
    fn an_unknown_value_reads_not_measured() {
        let found = record(vec![fact(LICENSE_SIGNAL, None)]);
        for text in [ChoiceText::of(Some(&found)), ChoiceText::of(None)] {
            assert_eq!(text.license, NOT_MEASURED);
            assert_eq!(text.runs_on, NOT_MEASURED);
            assert_eq!(text.active_users, NOT_MEASURED);
            assert_eq!(text.dependents, NOT_MEASURED);
        }
    }

    #[test]
    fn an_amount_that_is_no_number_reads_not_measured() {
        let found = record(vec![fact(ACTIVE_USERS_SIGNAL, Some("many"))]);
        assert_eq!(whole_text(Some(&found), ACTIVE_USERS_SIGNAL), NOT_MEASURED);
    }
}
