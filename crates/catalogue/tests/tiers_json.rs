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
        assert_eq!(entry.signals.len(), 33, "{slug}");
        for fact in &entry.signals {
            assert!(fact.problems().is_empty(), "{slug}: {:?}", fact.problems());
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
fn the_report_and_the_text_come_from_one_pass() {
    let (_, run) = offline_run();
    for (slug, entry) in &run.report.entries {
        let header = format!("slug: {slug}\ntier: {}\n", entry.tier.as_str());
        assert!(run.text.contains(&header), "{header} not in:\n{}", run.text);
    }
}
