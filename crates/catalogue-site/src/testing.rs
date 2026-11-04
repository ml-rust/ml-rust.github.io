//! Data shared by the unit tests.

use std::collections::BTreeMap;

use catalogue_core::{
    ACTIVE_USERS_SIGNAL, Catalogue, DOWNLOADS_SIGNAL, DataSources, Date, EntryReport,
    REPORT_SCHEMA, SignalFact, Slug, SourceFile, SourceRef, Tier, TierReport,
};
use url::Url;

/// One topic that every test entry names.
const TOPICS: &str = "[[topic]]\nid = \"one\"\nname = \"One\"\n";

pub(crate) fn source(path: &str, contents: String) -> SourceFile {
    SourceFile {
        path: path.to_owned(),
        contents,
    }
}

/// The slug `text`.
pub(crate) fn slug(text: &str) -> Slug {
    Slug::new(text).expect("slug")
}

/// An entry file for `name`, with `extra` as more top-level fields.
pub(crate) fn entry(name: &str, extra: &str) -> SourceFile {
    source(
        &format!("entries/{name}.toml"),
        format!(
            "{extra}name = \"{name}\"\nkind = \"library\"\nrepo = \"https://github.com/o/{name}\"\n\
             crates = [\"{name}\"]\ntopics = [\"one\"]\nadded = 2026-01-01\n\
             [description]\ntext = \"A thing.\"\nsource = \"human\"\n"
        ),
    )
}

/// The checked catalogue of `entries` over [`TOPICS`].
pub(crate) fn catalogue(entries: Vec<SourceFile>) -> Catalogue {
    Catalogue::load(DataSources {
        entries,
        topics: Some(source("topics.toml", TOPICS.to_owned())),
        ..DataSources::default()
    })
    .unwrap_or_else(|diagnostics| panic!("invalid data: {diagnostics}"))
}

/// A record with `tier`. A given `amount` is both its active dependents and its downloads.
pub(crate) fn record(tier: Tier, amount: Option<&str>) -> EntryReport {
    let signals = amount
        .into_iter()
        .flat_map(|value| {
            [ACTIVE_USERS_SIGNAL, DOWNLOADS_SIGNAL].map(|name| SignalFact {
                name: name.to_owned(),
                label: name.to_owned(),
                value: Some(value.to_owned()),
                source: Some(SourceRef::new(
                    "dump",
                    Url::parse("https://crates.io/data-access").expect("url"),
                )),
                fetched: Date::new(2026, 11, 2).ok(),
                counts_toward_tier: true,
            })
        })
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

/// A report of a check on 2026-11-03 that holds `records`.
pub(crate) fn report(records: Vec<(&str, EntryReport)>) -> TierReport {
    let entries: BTreeMap<Slug, EntryReport> = records
        .into_iter()
        .map(|(name, found)| (slug(name), found))
        .collect();
    TierReport {
        schema: REPORT_SCHEMA,
        generated: Date::new(2026, 11, 3).expect("date"),
        thresholds_version: 1,
        calibrated: None,
        entries,
    }
}
