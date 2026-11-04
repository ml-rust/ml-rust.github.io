#[path = "common/tiers_dump.rs"]
mod tiers_dump;

use catalogue::{Network, TiersRun, load_catalogue, tiers_report};
use catalogue_core::{Catalogue, Date, TierReport};
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::BuildChecks;
use tiers_dump::{fixture, snapshot};

/// The committed report of the offline run below.
const GOLDEN: &str = include_str!("golden/report.json");

fn day(year: i16, month: i8, day: i8) -> Date {
    Date::new(year, month, day).expect("date")
}

/// The offline run of the `valid` fixture on 2026-11-03, with a dated advisory archive.
fn offline_run() -> (Catalogue, TiersRun) {
    let catalogue = load_catalogue(&fixture("valid"))
        .expect("fixture reads")
        .expect("fixture is valid");
    let advisories = Advisories {
        archive_date: Some(day(2026, 11, 1)),
        ..Advisories::default()
    };
    let run = tiers_report(
        &catalogue,
        &snapshot(),
        &advisories,
        &BuildChecks::default(),
        Network::Offline,
        None,
        day(2026, 11, 3),
    )
    .expect("run");
    (catalogue, run)
}

#[test]
fn the_offline_report_matches_the_golden_file() {
    let (_, run) = offline_run();
    let mut text = serde_json::to_string_pretty(&run.report).expect("serializes");
    text.push('\n');
    if std::env::var_os("CATALOGUE_UPDATE_GOLDEN").is_some() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/report.json");
        std::fs::write(path, &text).expect("writes the golden file");
        return;
    }
    assert_eq!(text, GOLDEN);
}

#[test]
fn the_golden_file_reads_back_as_the_same_report() {
    let (catalogue, run) = offline_run();
    let back: TierReport = serde_json::from_str(GOLDEN).expect("parses");
    assert_eq!(back, run.report);
    back.check(&catalogue).expect("one record per entry");
}

#[test]
fn every_fact_cites_its_source_and_date() {
    let (_, run) = offline_run();
    for (slug, entry) in &run.report.entries {
        assert_eq!(entry.signals.len(), 40, "{slug}");
        for fact in &entry.signals {
            assert!(fact.problems().is_empty(), "{slug}: {:?}", fact.problems());
            assert_eq!(
                fact.counts_toward_tier,
                !catalogue_core::never_counts(&fact.name),
                "{slug}: {}",
                fact.name
            );
        }
        let stars = entry
            .signals
            .iter()
            .find(|fact| fact.name == "stars")
            .expect("stars row");
        assert!(!stars.counts_toward_tier, "{slug}");
    }
}

#[test]
fn the_choice_facts_read_the_dump_and_an_absent_crate_reads_as_not_measured() {
    let (_, run) = offline_run();
    let alpha = run
        .report
        .entries
        .iter()
        .find(|(slug, _)| slug.as_str() == "alpha")
        .map(|(_, entry)| entry)
        .expect("alpha");
    assert_eq!(alpha.value_of("license"), Some("Apache-2.0 OR MIT"));
    assert_eq!(alpha.value_of("runs_on"), Some("CUDA"));
    assert_eq!(alpha.value_of("native_code"), Some("Pure Rust"));
    assert_eq!(alpha.value_of("dependents"), Some("0"));
    assert_eq!(alpha.value_of("contributors"), None, "an offline run");
    let beta = run
        .report
        .entries
        .iter()
        .find(|(slug, _)| slug.as_str() == "beta")
        .map(|(_, entry)| entry)
        .expect("beta");
    for name in ["license", "runs_on", "native_code", "dependents"] {
        assert_eq!(beta.value_of(name), None, "{name}");
    }
}

#[test]
fn the_report_and_the_text_come_from_one_pass() {
    let (_, run) = offline_run();
    for (slug, entry) in &run.report.entries {
        let header = format!("slug: {slug}\ntier: {}\n", entry.tier.as_str());
        assert!(run.text.contains(&header), "{header} not in:\n{}", run.text);
    }
}
