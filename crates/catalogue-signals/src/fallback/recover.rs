//! Picks up the previous value of a failed fetch.

use catalogue_core::Date;

use super::source::Source;
use crate::history::Dated;

/// What a failed fetch leaves: the value that stands in, if any, and the entry note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovered<T> {
    /// The previous value with its original date. `None` when the signal reads as unknown.
    pub value: Option<Dated<T>>,
    /// The entry note. It names the source, and the date of the value in use.
    pub note: String,
}

/// The value `earlier` stands in for the failed fetch of `of` from `source`.
///
/// A value older than `max_age_days` on `today`, or one that describes another
/// subject, is no stand-in: the signal reads as unknown.
pub fn recover<T>(
    source: Source,
    earlier: Option<Dated<T>>,
    of: &str,
    error: &str,
    today: Date,
    max_age_days: u32,
) -> Recovered<T> {
    let unknown = |why: String| Recovered {
        value: None,
        note: format!("{source} failed and {why}: signal unknown. {error}"),
    };
    let Some(earlier) = earlier else {
        return unknown("no earlier snapshot holds a value".to_owned());
    };
    if earlier.of != of {
        return unknown(format!(
            "the earlier value describes {}, not {of}",
            earlier.of
        ));
    }
    if today.days_since(earlier.date) > i64::from(max_age_days) {
        return unknown(format!(
            "the value from {} is older than {max_age_days} days",
            earlier.date
        ));
    }
    let note = format!(
        "{source} failed: using the value from {}. {error}",
        earlier.date
    );
    Recovered {
        value: Some(earlier),
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(month: i8, on: i8) -> Date {
        Date::new(2026, month, on).expect("date")
    }

    fn earlier(date: Date, of: &str) -> Option<Dated<u64>> {
        Some(Dated {
            date,
            of: of.to_owned(),
            value: 7,
        })
    }

    fn run(found: Option<Dated<u64>>) -> Recovered<u64> {
        recover(Source::Homebrew, found, "tool", "boom", day(11, 15), 14)
    }

    #[test]
    fn a_value_inside_the_limit_keeps_its_date_and_names_it() {
        let found = run(earlier(day(11, 1), "tool"));
        assert_eq!(found.value.as_ref().map(|v| v.date), Some(day(11, 1)));
        assert_eq!(found.value.as_ref().map(|v| v.value), Some(7));
        assert_eq!(
            found.note,
            "Homebrew failed: using the value from 2026-11-01. boom"
        );
    }

    #[test]
    fn a_value_past_the_limit_is_unknown() {
        let found = run(earlier(day(10, 31), "tool"));
        assert_eq!(found.value, None);
        assert_eq!(
            found.note,
            "Homebrew failed and the value from 2026-10-31 is older than 14 days: signal unknown. boom"
        );
    }

    #[test]
    fn no_earlier_value_is_unknown_with_a_note() {
        let found = run(None);
        assert_eq!(found.value, None);
        assert_eq!(
            found.note,
            "Homebrew failed and no earlier snapshot holds a value: signal unknown. boom"
        );
    }

    #[test]
    fn a_value_about_another_subject_is_unknown() {
        let found = run(earlier(day(11, 14), "old-tool"));
        assert_eq!(found.value, None);
        assert!(
            found.note.contains("describes old-tool, not tool"),
            "{}",
            found.note
        );
    }
}
