//! Other entries that share a topic with one entry, as rows that compare with it.
//!
//! A shared topic is the whole claim behind the section heading. The table caption says so.

use std::collections::{BTreeMap, BTreeSet};

use catalogue_core::{EffectiveEntry, Slug};

use crate::inputs::SiteInputs;
use crate::pages::shared::{
    EntryRow, FamilyHead, RelatedLink, family_heads, movement_word, tier_of, topic_link,
};

/// The most rows the section shows.
const LIMIT: usize = 5;

/// The comparison row of `slug`, with the facts of its record.
fn row<'a>(inputs: &SiteInputs<'a>, slug: &Slug) -> Option<EntryRow<'a>> {
    let entry = inputs.catalogue.entry(slug)?;
    let report = inputs.report;
    let record = report.and_then(|report| report.entries.get(slug));
    Some(EntryRow::new(slug, entry, tier_of(report, slug)).with_choice(record))
}

/// Up to [`LIMIT`] rows of the other entries that share a topic with `effective`.
///
/// Rows follow the one order of every list, with one row per project family.
pub(crate) fn also_in<'a>(
    inputs: &SiteInputs<'a>,
    effective: &EffectiveEntry<'_>,
) -> Vec<EntryRow<'a>> {
    let catalogue = inputs.catalogue;
    let mut shared: BTreeMap<&'a Slug, &Slug> = BTreeMap::new();
    for topic in effective.topics {
        for slug in catalogue.entries_in_topic(topic) {
            if slug != effective.slug {
                shared.entry(slug).or_insert(topic);
            }
        }
    }
    let slugs: Vec<&'a Slug> = shared.keys().copied().collect();
    let heads = family_heads(catalogue, inputs.report, &slugs);
    let picked = fair_share(&heads, &shared, effective.topics);
    let jobs: BTreeSet<&Slug> = picked
        .iter()
        .filter_map(|slug| shared.get(slug).copied())
        .collect();
    heads
        .into_iter()
        .filter(|head| picked.contains(head.slug))
        .filter_map(|head| {
            let found = row(inputs, head.slug)?.with_family(
                movement_word(inputs, head.slug),
                RelatedLink::of(head.slug, head.related),
            );
            // Rows from one job need no line each: the page already names that job.
            if jobs.len() < 2 {
                return Some(found);
            }
            let topic = topic_link(catalogue, shared.get(head.slug)?)?;
            Some(found.with_also_in(topic))
        })
        .collect()
}

/// Up to [`LIMIT`] of `heads`, taken from each topic of `topics` in turn.
///
/// A big second topic never pushes out the rows of the page entry's first topic.
fn fair_share<'s>(
    heads: &[FamilyHead<'s>],
    shared: &BTreeMap<&'s Slug, &Slug>,
    topics: &[Slug],
) -> BTreeSet<&'s Slug> {
    let mut queues: Vec<Vec<&'s Slug>> = topics
        .iter()
        .map(|topic| {
            heads
                .iter()
                .map(|head| head.slug)
                .filter(|slug| shared.get(slug).is_some_and(|first| *first == topic))
                .rev()
                .collect()
        })
        .collect();
    let mut picked = BTreeSet::new();
    while picked.len() < LIMIT && queues.iter().any(|queue| !queue.is_empty()) {
        for queue in &mut queues {
            if picked.len() < LIMIT
                && let Some(slug) = queue.pop()
            {
                picked.insert(slug);
            }
        }
    }
    picked
}

/// The comparison rows of the page of `effective`: its own row first, then [`also_in`].
///
/// Empty when no other entry shares a topic, so the section stays out.
pub(crate) fn compare_rows<'a>(
    inputs: &SiteInputs<'a>,
    effective: &EffectiveEntry<'_>,
) -> Vec<EntryRow<'a>> {
    let others = also_in(inputs, effective);
    if others.is_empty() {
        return others;
    }
    row(inputs, effective.slug)
        .map(EntryRow::marked_current)
        .into_iter()
        .chain(others)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use catalogue_core::{
        Catalogue, DOWNLOADS_SIGNAL, DataSources, Date, EntryReport, Movement, REPORT_SCHEMA,
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

    fn record(tier: Tier, downloads: Option<&str>) -> EntryReport {
        let signals = downloads
            .map(|value| SignalFact {
                name: DOWNLOADS_SIGNAL.to_owned(),
                label: "Downloads over 30 days".to_owned(),
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
            search_terms: Vec::new(),
            readme: None,
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
            .map(|row| row.name)
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
            .map(|row| {
                let topic = row.also_in.as_ref().expect("a shared topic");
                (row.name, topic.name)
            })
            .collect();
        assert_eq!(topics, [("both", "Two"), ("only-one", "One")]);
        assert!(rows.iter().all(|row| row.badge.is_none()));
    }

    #[test]
    fn rows_from_one_job_drop_the_line_that_names_it() {
        let data = catalogue(vec![
            entry("page", "\"one\", \"two\""),
            entry("first", "\"one\""),
            entry("second", "\"one\""),
        ]);
        let slug = Slug::new("page").expect("slug");
        let effective = data.effective(&slug).expect("entry");
        let rows = also_in(&inputs(&data, None), &effective);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.also_in.is_none()));
    }

    #[test]
    fn a_big_second_topic_never_pushes_out_the_first_topic() {
        let mut entries = vec![entry("page", "\"one\", \"two\""), entry("small", "\"one\"")];
        for name in ["big-a", "big-b", "big-c", "big-d", "big-e", "big-f"] {
            entries.push(entry(name, "\"two\""));
        }
        let data = catalogue(entries);
        let tiers = report(vec![
            ("small", record(Tier::Listed, Some("1"))),
            ("big-a", record(Tier::Featured, Some("90"))),
            ("big-b", record(Tier::Featured, Some("80"))),
            ("big-c", record(Tier::Featured, Some("70"))),
            ("big-d", record(Tier::Featured, Some("60"))),
            ("big-e", record(Tier::Featured, Some("50"))),
            ("big-f", record(Tier::Featured, Some("40"))),
        ]);
        assert_eq!(
            names(&data, Some(&tiers), "page"),
            ["big-a", "big-b", "big-c", "big-d", "small"],
            "the one order of every list holds inside the five rows"
        );
    }

    #[test]
    fn the_comparison_leads_with_the_page_entry_and_keeps_the_list_facts() {
        let data = catalogue(vec![entry("page", "\"one\""), entry("other", "\"one\"")]);
        let tiers = report(vec![
            ("page", record(Tier::Listed, Some("2"))),
            ("other", record(Tier::Featured, Some("1200.4"))),
        ]);
        let slug = Slug::new("page").expect("slug");
        let effective = data.effective(&slug).expect("entry");
        let rows = compare_rows(&inputs(&data, Some(&tiers)), &effective);
        let names: Vec<(&str, bool)> = rows.iter().map(|row| (row.name, row.current)).collect();
        assert_eq!(names, [("page", true), ("other", false)]);
        assert_eq!(rows[0].also_in, None);
        let facts = rows[1].choice.as_ref().expect("facts");
        assert_eq!(facts.downloads_cell.short, "1.2k");
    }

    #[test]
    fn with_no_other_entry_the_comparison_stays_empty() {
        let data = catalogue(vec![entry("page", "\"one\"")]);
        let slug = Slug::new("page").expect("slug");
        let effective = data.effective(&slug).expect("entry");
        assert!(compare_rows(&inputs(&data, None), &effective).is_empty());
    }

    #[test]
    fn an_entry_with_no_shared_topic_has_no_row() {
        let data = catalogue(vec![entry("page", "\"one\""), entry("other", "\"two\"")]);
        assert!(names(&data, None, "page").is_empty());
    }
}
