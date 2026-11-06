//! A build with a tier report refuses data that cannot publish tiers.

#[path = "common/build.rs"]
mod build;
mod common;
#[path = "common/report.rs"]
mod report;

use catalogue_core::{Catalogue, Movement, REPORT_SCHEMA, Slug, TierReport};
use catalogue_site::{OutputFile, SiteError, SiteInputs, build_site};
use common::{TOPICS, catalogue, pages, thresholds};
use report::report;

/// The markup a page uses to show an entry's tier.
const TIER_MARKUP: [&str; 2] = ["tier-badge", "data-tier"];

fn build(catalogue: &Catalogue, report: &TierReport) -> Result<Vec<OutputFile>, SiteError> {
    build_site(&SiteInputs {
        catalogue,
        report: Some(report),
        changes: &[],
        first_check: None,
        movement: &Movement::NeedsHistory,
        state: None,
    })
}

fn calibrated() -> Catalogue {
    catalogue(TOPICS, Some(&thresholds(Some("2026-11-03"))))
}

#[test]
fn a_matching_report_builds() {
    let data = calibrated();
    let files = build(&data, &report(&data, &["alpha"])).expect("builds");
    assert!(files.iter().any(|file| file.path == "rules/index.html"));
}

#[test]
fn a_report_without_thresholds_is_refused() {
    let tiers = report(&calibrated(), &["alpha"]);
    let data = catalogue(TOPICS, None);
    let error = build(&data, &tiers).expect_err("refused");
    assert!(matches!(error, SiteError::NoThresholds), "{error:?}");
    assert!(
        error.to_string().contains("data/thresholds.toml"),
        "{error}"
    );
}

#[test]
fn a_report_over_uncalibrated_thresholds_is_refused() {
    let data = catalogue(TOPICS, Some(&thresholds(None)));
    let error = build(&data, &report(&data, &["alpha"])).expect_err("refused");
    assert!(matches!(error, SiteError::Uncalibrated), "{error:?}");
    let message = error.to_string();
    assert!(message.contains("data/thresholds.toml"), "{message}");
    assert!(message.contains("`calibrated`"), "{message}");
}

#[test]
fn a_report_in_another_schema_is_refused() {
    let data = calibrated();
    let mut tiers = report(&data, &["alpha"]);
    tiers.schema = REPORT_SCHEMA + 1;
    let error = build(&data, &tiers).expect_err("refused");
    assert!(
        matches!(error, SiteError::ReportSchema { found, expected } if found == REPORT_SCHEMA + 1 && expected == REPORT_SCHEMA),
        "{error:?}"
    );
}

#[test]
fn a_report_over_another_thresholds_version_is_refused() {
    let data = calibrated();
    let mut tiers = report(&data, &["alpha"]);
    tiers.thresholds_version = 2;
    let error = build(&data, &tiers).expect_err("refused");
    assert!(
        matches!(error, SiteError::StaleReport { report: 2, data: 3 }),
        "{error:?}"
    );
    assert!(
        error.to_string().contains("catalogue tiers --json"),
        "{error}"
    );
}

#[test]
fn a_report_over_other_entries_is_refused() {
    let data = calibrated();
    let error = build(&data, &report(&data, &["beta"])).expect_err("refused");
    let SiteError::ReportMismatch(mismatch) = error else {
        panic!("expected a mismatch, got {error:?}");
    };
    let slug = |text: &str| Slug::new(text).expect("slug");
    assert_eq!(mismatch.missing, [slug("alpha")]);
    assert_eq!(mismatch.extra, [slug("beta")]);
}

#[test]
fn without_a_report_every_page_builds_with_no_tier_shown() {
    for data in [
        catalogue(TOPICS, None),
        catalogue(TOPICS, Some(&thresholds(None))),
        calibrated(),
    ] {
        let site = pages(&data);
        assert!(site.contains_key("rules/index.html"));
        for (path, text) in site.iter().filter(|(path, _)| path.ends_with(".html")) {
            for markup in TIER_MARKUP {
                assert!(!text.contains(markup), "{path} shows a tier: {markup}");
            }
        }
    }
}
