//! `data/catalogue.json`: every entry as the pages show it.
//!
//! The file holds only what the pages show. A fact without a value carries no source and no
//! date. A link that `SafeHref` refuses reads as null. Topics are the effective topics.

use catalogue_core::{
    CrateName, DescriptionSource, EffectiveEntry, EntryKind, EntryReport, ReasonRecord, SignalFact,
    Slug, Thresholds, Tier,
};
use serde::Serialize;
use url::Url;

use crate::build::OutputFile;
use crate::error::SiteError;
use crate::href::SafeHref;
use crate::inputs::SiteInputs;
use crate::pages::{ChoiceText, WhyText, docs_href, install_line};

/// The path of the file below the site root.
pub(crate) const CATALOGUE_FILE: &str = "data/catalogue.json";

/// The version of the file format. A change to any field raises it.
pub(crate) const CATALOGUE_SCHEMA: u32 = 2;

/// The license of the data.
pub(crate) const DATA_LICENSE: &str = "CC-BY-4.0";

#[derive(Debug, Serialize)]
struct CatalogueFile<'a> {
    schema: u32,
    /// The run date of the check behind the tiers. Null without a report.
    generated: Option<String>,
    license: &'static str,
    /// The thresholds version the check applied. Null without a report.
    thresholds_version: Option<u32>,
    entries: Vec<EntryRecord<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct EntryRecord<'a> {
    pub(super) slug: &'a Slug,
    pub(super) name: &'a str,
    pub(super) kind: EntryKind,
    pub(super) repo: Option<String>,
    crates: &'a [CrateName],
    topics: &'a [Slug],
    category: Option<&'a Slug>,
    part_of: Option<&'a Slug>,
    pub(super) description: DescriptionRecord<'a>,
    pub(super) install: InstallRecord,
    pub(super) docs: Option<String>,
    pub(super) tier: Option<Tier>,
    pub(super) why: Option<WhyRecord>,
    pub(super) native_word: String,
    reasons: &'a [ReasonRecord],
    pub(super) signals: Vec<SignalRecord<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct InstallRecord {
    command: Option<String>,
    text: &'static str,
    url: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct WhyRecord {
    verdict: String,
    lead: Vec<String>,
    meets: Vec<String>,
    lacks: Vec<String>,
    found: Vec<String>,
}

impl From<WhyText> for WhyRecord {
    fn from(text: WhyText) -> Self {
        Self {
            verdict: text.verdict,
            lead: text.lead,
            meets: text.meets,
            lacks: text.lacks,
            found: text.found,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct DescriptionRecord<'a> {
    text: &'a str,
    source: DescriptionSource,
    source_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct SignalRecord<'a> {
    pub(super) name: &'a str,
    label: &'a str,
    value: Option<&'a str>,
    source: Option<SourceRecord<'a>>,
    fetched: Option<String>,
    counts_toward_tier: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct SourceRecord<'a> {
    label: &'a str,
    url: Option<String>,
}

/// The text of `url`, or `None` when [`SafeHref`] refuses it.
fn safe_url(url: &Url) -> Option<String> {
    href_text(&SafeHref::from(url))
}

/// The text of `href`, or `None` when it is refused.
fn href_text(href: &SafeHref) -> Option<String> {
    (!href.is_refused()).then(|| href.as_str().to_owned())
}

impl<'a> SignalRecord<'a> {
    fn of(fact: &'a SignalFact) -> Self {
        let measured = fact.value.is_some();
        Self {
            name: &fact.name,
            label: &fact.label,
            value: fact.value.as_deref(),
            source: fact
                .source
                .as_ref()
                .filter(|_| measured)
                .map(|source| SourceRecord {
                    label: &source.label,
                    url: safe_url(&source.url),
                }),
            fetched: fact
                .fetched
                .filter(|_| measured)
                .map(|date| date.to_string()),
            counts_toward_tier: fact.counts_toward_tier,
        }
    }
}

impl<'a> EntryRecord<'a> {
    fn of(
        effective: &EffectiveEntry<'a>,
        record: Option<&'a EntryReport>,
        thresholds: Option<&Thresholds>,
    ) -> Self {
        let entry = effective.entry;
        let install = install_line(entry);
        Self {
            slug: effective.slug,
            name: &entry.name,
            kind: entry.kind,
            repo: safe_url(&entry.repo),
            crates: &entry.crates,
            topics: effective.topics,
            category: entry.category.as_ref(),
            part_of: entry.part_of.as_ref(),
            description: DescriptionRecord {
                text: &entry.description.text,
                source: entry.description.source,
                source_url: entry.description.source_url.as_ref().and_then(safe_url),
            },
            install: InstallRecord {
                command: install.command,
                text: install.text,
                url: href_text(&install.href),
            },
            docs: docs_href(entry).as_ref().and_then(href_text),
            tier: record.map(|record| record.tier),
            why: record.map(|record| WhyText::of(record, entry.kind, thresholds).into()),
            native_word: ChoiceText::of(record).native_code.short,
            reasons: record
                .map(|record| record.reasons.as_slice())
                .unwrap_or_default(),
            signals: record
                .map(|record| record.signals.iter().map(SignalRecord::of).collect())
                .unwrap_or_default(),
        }
    }
}

/// Every entry of `inputs` as the data files print it, sorted by slug.
pub(super) fn entry_records<'a>(inputs: &SiteInputs<'a>) -> Vec<EntryRecord<'a>> {
    let catalogue = inputs.catalogue;
    let report = inputs.report;
    catalogue
        .entries()
        .keys()
        .filter_map(|slug| catalogue.effective(slug))
        .map(|effective| {
            let record = report.and_then(|report| report.entries.get(effective.slug));
            EntryRecord::of(&effective, record, catalogue.thresholds())
        })
        .collect()
}

/// `value` as the JSON file at `path`.
///
/// `<`, `>` and `&` are written as `\u` escapes, so the file is safe to embed in a page.
///
/// # Errors
///
/// Returns [`SiteError::Encode`] when the JSON fails to encode.
pub(super) fn json_file(path: String, value: &impl Serialize) -> Result<OutputFile, SiteError> {
    let text = match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(source) => return Err(SiteError::Encode { path, source }),
    };
    let mut escaped = String::with_capacity(text.len() + 1);
    for character in text.chars() {
        match character {
            '<' => escaped.push_str("\\u003c"),
            '>' => escaped.push_str("\\u003e"),
            '&' => escaped.push_str("\\u0026"),
            other => escaped.push(other),
        }
    }
    escaped.push('\n');
    Ok(OutputFile {
        path,
        bytes: escaped.into_bytes(),
    })
}

/// The data file of `inputs`. The same inputs give the same bytes.
///
/// # Errors
///
/// Returns [`SiteError::Encode`] when the JSON fails to encode.
pub(crate) fn catalogue_file(inputs: &SiteInputs<'_>) -> Result<OutputFile, SiteError> {
    let report = inputs.report;
    let file = CatalogueFile {
        schema: CATALOGUE_SCHEMA,
        generated: report.map(|report| report.generated.to_string()),
        license: DATA_LICENSE,
        thresholds_version: report.map(|report| report.thresholds_version),
        entries: entry_records(inputs),
    };
    json_file(CATALOGUE_FILE.to_owned(), &file)
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Catalogue, Date, Movement, SourceRef, TierReport};
    use serde_json::Value;

    use super::*;
    use crate::testing::{catalogue, entry, record, report};

    static NO_MOVEMENT: Movement = Movement::NeedsHistory;

    fn built(data: &Catalogue, tiers: Option<&TierReport>) -> OutputFile {
        catalogue_file(&SiteInputs {
            catalogue: data,
            report: tiers,
            changes: &[],
            first_check: None,
            movement: &NO_MOVEMENT,
        })
        .expect("encodes")
    }

    fn parsed(file: &OutputFile) -> Value {
        serde_json::from_slice(&file.bytes).expect("valid JSON")
    }

    #[test]
    fn the_file_holds_the_schema_the_date_and_every_entry_field() {
        let data = catalogue(vec![
            entry("core", ""),
            entry("core-a", "part_of = \"core\"\n"),
        ]);
        let tiers = report(vec![
            ("core", record(Tier::Featured, Some("12.5"))),
            ("core-a", record(Tier::Listed, None)),
        ]);
        let file = built(&data, Some(&tiers));
        assert_eq!(file.path, "data/catalogue.json");
        let value = parsed(&file);
        assert_eq!(value["schema"], CATALOGUE_SCHEMA);
        assert_eq!(value["generated"], "2026-11-03");
        assert_eq!(value["license"], "CC-BY-4.0");
        assert_eq!(value["thresholds_version"], 1);
        let core = &value["entries"][0];
        assert_eq!(core["slug"], "core");
        assert_eq!(core["name"], "core");
        assert_eq!(core["kind"], "library");
        assert_eq!(core["repo"], "https://github.com/o/core");
        assert_eq!(core["crates"][0], "core");
        assert_eq!(core["topics"][0], "one");
        assert_eq!(core["category"], Value::Null);
        assert_eq!(core["part_of"], Value::Null);
        assert_eq!(core["description"]["text"], "A thing.");
        assert_eq!(core["description"]["source"], "human");
        assert_eq!(core["description"]["source_url"], Value::Null);
        assert_eq!(core["tier"], "featured");
        assert_eq!(core["install"]["command"], "cargo add core");
        assert_eq!(core["install"]["text"], "crates.io");
        assert_eq!(core["install"]["url"], "https://crates.io/crates/core");
        assert_eq!(core["docs"], "https://docs.rs/core");
        assert_eq!(core["native_word"], "Not measured");
        assert_eq!(core["why"]["lead"][0], "It passes every Featured gate.");
        assert!(core["reasons"].as_array().expect("reasons").is_empty());
        let signal = &core["signals"][0];
        assert_eq!(signal["name"], "active_users");
        assert_eq!(signal["label"], "Active users");
        assert_eq!(signal["value"], "12.5");
        assert_eq!(signal["source"]["label"], "dump");
        assert_eq!(signal["source"]["url"], "https://crates.io/data-access");
        assert_eq!(signal["fetched"], "2026-11-02");
        assert_eq!(signal["counts_toward_tier"], true);
        assert_eq!(value["entries"][1]["part_of"], "core");
    }

    #[test]
    fn the_same_inputs_give_the_same_bytes() {
        let data = catalogue(vec![entry("zeta", ""), entry("alpha", "")]);
        let tiers = report(vec![
            ("zeta", record(Tier::Listed, Some("1"))),
            ("alpha", record(Tier::New, None)),
        ]);
        assert_eq!(built(&data, Some(&tiers)), built(&data, Some(&tiers)));
        let value = parsed(&built(&data, Some(&tiers)));
        assert_eq!(value["entries"][0]["slug"], "alpha");
    }

    #[test]
    fn without_a_report_the_file_holds_no_tier_and_no_signal() {
        let data = catalogue(vec![entry("alpha", "")]);
        let value = parsed(&built(&data, None));
        assert_eq!(value["generated"], Value::Null);
        assert_eq!(value["thresholds_version"], Value::Null);
        let alpha = &value["entries"][0];
        assert_eq!(alpha["tier"], Value::Null);
        assert_eq!(alpha["why"], Value::Null);
        assert!(alpha["reasons"].as_array().expect("reasons").is_empty());
        assert!(alpha["signals"].as_array().expect("signals").is_empty());
    }

    #[test]
    fn a_fact_without_a_value_hides_its_source_and_date_as_the_page_does() {
        let data = catalogue(vec![entry("alpha", "")]);
        let mut found = record(Tier::Listed, Some("3"));
        found.signals[0].value = None;
        let tiers = report(vec![("alpha", found)]);
        let value = parsed(&built(&data, Some(&tiers)));
        let signal = &value["entries"][0]["signals"][0];
        assert_eq!(signal["value"], Value::Null);
        assert_eq!(signal["source"], Value::Null);
        assert_eq!(signal["fetched"], Value::Null);
    }

    #[test]
    fn a_link_the_pages_refuse_reads_as_null() {
        let data = catalogue(vec![entry("alpha", "")]);
        let mut found = record(Tier::Listed, Some("3"));
        found.signals[0].source = Some(SourceRef::new(
            "script",
            Url::parse("javascript:alert(1)").expect("url"),
        ));
        found.signals[0].fetched = Date::new(2026, 11, 2).ok();
        let tiers = report(vec![("alpha", found)]);
        let value = parsed(&built(&data, Some(&tiers)));
        let source = &value["entries"][0]["signals"][0]["source"];
        assert_eq!(source["label"], "script");
        assert_eq!(source["url"], Value::Null);
    }

    #[test]
    fn markup_in_a_value_stays_escaped_and_parses_back() {
        let data = catalogue(vec![entry("alpha", "")]);
        let mut found = record(Tier::Listed, Some("3"));
        found.signals[0].label = "<script>&".to_owned();
        let tiers = report(vec![("alpha", found)]);
        let file = built(&data, Some(&tiers));
        let text = String::from_utf8(file.bytes.clone()).expect("UTF-8");
        assert!(!text.contains("<script"), "{text}");
        let value = parsed(&file);
        assert_eq!(value["entries"][0]["signals"][0]["label"], "<script>&");
    }
}
