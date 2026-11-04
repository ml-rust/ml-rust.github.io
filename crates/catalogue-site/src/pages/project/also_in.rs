//! Other entries that share a topic with one entry.
//!
//! A shared topic is the whole claim. The section says "Also in" and nothing about similarity.

use std::collections::BTreeMap;

use catalogue_core::{EffectiveEntry, Slug};

use crate::inputs::SiteInputs;
use crate::pages::shared::{
    NamedLink, RelatedLink, TierBadge, entry_link, family_heads, movement_word, tier_of, topic_link,
};

/// The most rows the section shows.
const LIMIT: usize = 5;

/// One entry that shares a topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AlsoInRow<'a> {
    /// The link to the entry.
    pub entry: NamedLink<'a>,
    /// The tier badge. `None` without a report.
    pub badge: Option<TierBadge>,
    /// The link to the first topic of the page's entry that this entry shares.
    pub topic: NamedLink<'a>,
    /// The plain word for how the entry moved.
    pub movement: Option<&'static str>,
    /// The link that stands for the related projects this row hides.
    pub related: Option<RelatedLink>,
}

/// Up to [`LIMIT`] rows of the other entries that share a topic with `effective`.
///
/// Rows follow the one order of every list, with one row per project family.
pub(crate) fn also_in<'a>(
    inputs: &SiteInputs<'a>,
    effective: &EffectiveEntry<'_>,
) -> Vec<AlsoInRow<'a>> {
    let catalogue = inputs.catalogue;
    let report = inputs.report;
    let mut shared: BTreeMap<&'a Slug, &Slug> = BTreeMap::new();
    for topic in effective.topics {
        for slug in catalogue.entries_in_topic(topic) {
            if slug != effective.slug {
                shared.entry(slug).or_insert(topic);
            }
        }
    }
    let slugs: Vec<&'a Slug> = shared.keys().copied().collect();
    family_heads(catalogue, report, &slugs)
        .into_iter()
        .filter_map(|head| {
            let topic = shared.get(head.slug)?;
            Some(AlsoInRow {
                entry: entry_link(catalogue, head.slug)?,
                badge: tier_of(report, head.slug).map(TierBadge::of),
                topic: topic_link(catalogue, topic)?,
                movement: movement_word(inputs, head.slug),
                related: RelatedLink::of(head.slug, head.related),
            })
        })
        .take(LIMIT)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use catalogue_core::{
        ACTIVE_USERS_SIGNAL, Catalogue, DataSources, Date, EntryReport, Movement, REPORT_SCHEMA,
        SignalFact, SourceFile, SourceRef, Tier, TierReport,
    };
    use url::Url;

    use super::*;

    static NO_MOVEMENT: Movement = Movement::NeedsHistory;

    fn inputs<'a>(data: &'a Catalogue, report: Option<&'a TierReport>) -> SiteInputs<'a> {
        SiteInputs {
            catalogue: data,
            report,
            changes: &[],
            first_check: None,
            movement: &NO_MOVEMENT,
        }
    }

    const TOPICS: &str = "[[topic]]\nid = \"one\"\nname = \"One\"\n\n[[topic]]\nid = \"two\"\nname = \"Two\"\n\n[[topic]]\nid = \"three\"\nname = \"Three\"\n";

    fn source(path: &str, contents: String) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            contents,
        }
    }

    /// An entry file for `slug` in `topics`.
    fn entry(slug: &str, topics: &str) -> SourceFile {
        source(
            &format!("entries/{slug}.toml"),
            format!(
                "name = \"{slug}\"\nkind = \"library\"\nrepo = \"https://github.com/o/{slug}\"\n\
                 crates = [\"{slug}\"]\ntopics = [{topics}]\nadded = 2026-01-01\n\
                 [description]\ntext = \"A thing.\"\nsource = \"human\"\n"
            ),
        )
    }

    fn catalogue(entries: Vec<SourceFile>) -> Catalogue {
        Catalogue::load(DataSources {
            entries,
            topics: Some(source("topics.toml", TOPICS.to_owned())),
            ..DataSources::default()
        })
        .unwrap_or_else(|diagnostics| panic!("invalid data: {diagnostics}"))
    }

    fn record(tier: Tier, users: Option<&str>) -> EntryReport {
        let signals = users
            .map(|value| SignalFact {
                name: ACTIVE_USERS_SIGNAL.to_owned(),
                label: "Active users".to_owned(),
                value: Some(value.to_owned()),
                source: Some(SourceRef::new(
                    "dump",
                    Url::parse("https://crates.io/data-access").expect("url"),
                )),
                fetched: Date::new(2026, 11, 2).ok(),
                counts_toward_tier: true,
            })
            .into_iter()
            .collect();
        EntryReport {
            tier,
            stable: false,
            dormant: None,
            demotion_pending: None,
            reasons: Vec::new(),
            unknown: Vec::new(),
            notes: Vec::new(),
            signals,
        }
    }

    fn report(records: Vec<(&str, EntryReport)>) -> TierReport {
        let entries: BTreeMap<Slug, EntryReport> = records
            .into_iter()
            .map(|(slug, record)| (Slug::new(slug).expect("slug"), record))
            .collect();
        TierReport {
            schema: REPORT_SCHEMA,
            generated: Date::new(2026, 11, 3).expect("date"),
            thresholds_version: 1,
            calibrated: None,
            entries,
        }
    }

    fn names<'a>(data: &'a Catalogue, report: Option<&'a TierReport>, of: &str) -> Vec<&'a str> {
        let slug = Slug::new(of).expect("slug");
        let effective = data.effective(&slug).expect("entry");
        also_in(&inputs(data, report), &effective)
            .iter()
            .map(|row| row.entry.name)
            .collect()
    }

    #[test]
    fn the_strongest_tier_comes_first_then_the_most_users_then_the_name() {
        let data = catalogue(vec![
            entry("page", "\"one\""),
            entry("archived", "\"one\""),
            entry("fewer", "\"one\""),
            entry("more", "\"one\""),
            entry("featured", "\"one\""),
            entry("tied-b", "\"one\""),
            entry("tied-a", "\"one\""),
        ]);
        let tiers = report(vec![
            ("archived", record(Tier::Archived, Some("999"))),
            ("fewer", record(Tier::Listed, Some("2.5"))),
            ("more", record(Tier::Listed, Some("10"))),
            ("featured", record(Tier::Featured, None)),
            ("tied-a", record(Tier::New, Some("1"))),
            ("tied-b", record(Tier::New, Some("1"))),
        ]);
        assert_eq!(
            names(&data, Some(&tiers), "page"),
            ["featured", "more", "fewer", "tied-a", "tied-b"],
            "at most five rows, and the entry itself never shows"
        );
    }

    #[test]
    fn a_row_names_the_first_shared_topic_of_the_page_entry() {
        let data = catalogue(vec![
            entry("page", "\"two\", \"one\""),
            entry("both", "\"one\", \"two\""),
            entry("only-one", "\"one\""),
        ]);
        let slug = Slug::new("page").expect("slug");
        let effective = data.effective(&slug).expect("entry");
        let rows = also_in(&inputs(&data, None), &effective);
        let topics: Vec<(&str, &str)> = rows
            .iter()
            .map(|row| (row.entry.name, row.topic.name))
            .collect();
        assert_eq!(topics, [("both", "Two"), ("only-one", "One")]);
        assert!(rows.iter().all(|row| row.badge.is_none()));
    }

    #[test]
    fn an_entry_with_no_shared_topic_has_no_row() {
        let data = catalogue(vec![entry("page", "\"one\""), entry("other", "\"two\"")]);
        assert!(names(&data, None, "page").is_empty());
    }
}
