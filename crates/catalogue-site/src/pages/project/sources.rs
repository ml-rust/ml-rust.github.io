//! Where the facts of one block come from: each distinct source with its fetch date.

use catalogue_core::{EntryReport, SignalFact};

use crate::href::SafeHref;
use crate::pages::shared::date_text;

/// One source of a block, with the date of the data behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceNote<'a> {
    /// The source name.
    pub name: &'a str,
    /// The link to the source.
    pub href: SafeHref,
    /// The fetch date as an ISO date. `None` when the fact holds no date.
    pub fetched: Option<String>,
    /// The fetch date as a person reads it. `None` when the fact holds no date.
    pub fetched_text: Option<String>,
}

impl<'a> SourceNote<'a> {
    /// The source of `fact`. `None` for a fact with no value or no source.
    pub(super) fn of(fact: &'a SignalFact) -> Option<Self> {
        fact.value.as_ref()?;
        let source = fact.source.as_ref()?;
        let fetched = fact.fetched.map(|date| date.to_string());
        Some(Self {
            name: &source.label,
            href: SafeHref::from(&source.url),
            fetched_text: fetched.as_deref().and_then(date_text),
            fetched,
        })
    }
}

/// The fact `name` of `record`. `None` when the record holds no such fact.
pub(super) fn fact_named<'a>(record: &'a EntryReport, name: &str) -> Option<&'a SignalFact> {
    record.signals.iter().find(|fact| fact.name == name)
}

/// Each distinct source of `facts`, in order of first appearance.
pub(super) fn distinct_sources<'a>(
    facts: impl IntoIterator<Item = &'a SignalFact>,
) -> Vec<SourceNote<'a>> {
    let mut out: Vec<SourceNote<'a>> = Vec::new();
    for note in facts.into_iter().filter_map(SourceNote::of) {
        if !out.contains(&note) {
            out.push(note);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, SourceRef};
    use url::Url;

    use super::*;

    fn fact(value: Option<&str>, source: &str) -> SignalFact {
        SignalFact {
            name: "a".to_owned(),
            label: "A".to_owned(),
            value: value.map(str::to_owned),
            source: Some(SourceRef::new(
                source,
                Url::parse("https://crates.io/data-access").expect("url"),
            )),
            fetched: Date::new(2026, 11, 3).ok(),
            counts_toward_tier: true,
        }
    }

    #[test]
    fn a_source_prints_once_and_a_fact_with_no_value_gives_none() {
        let facts = [
            fact(Some("1"), "dump"),
            fact(Some("2"), "dump"),
            fact(None, "unseen"),
            fact(Some("3"), "other"),
        ];
        let notes = distinct_sources(&facts);
        let names: Vec<&str> = notes.iter().map(|note| note.name).collect();
        assert_eq!(names, ["dump", "other"]);
        assert_eq!(notes[0].fetched_text.as_deref(), Some("3 Nov 2026"));
    }
}
