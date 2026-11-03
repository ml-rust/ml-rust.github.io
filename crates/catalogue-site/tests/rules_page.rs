//! The rules page states the tier principles, every threshold and the calibration state.

#[path = "common/build.rs"]
mod build;
mod common;

use common::{TOPICS, catalogue, pages, thresholds};

fn rules(thresholds: Option<&str>) -> String {
    let mut site = pages(&catalogue(TOPICS, thresholds));
    site.remove("rules/index.html").expect("rules page")
}

#[test]
fn the_page_states_the_tier_principles_and_the_rot_signals() {
    let page = rules(Some(&thresholds(Some("2026-11-03"))));
    for text in [
        "Tiers come from fixed thresholds over raw signals. There is no composite score.",
        "Stars never count toward a tier.",
        "Thresholds version 3.",
        "Calibrated on 2026-11-03.",
    ] {
        assert!(page.contains(text), "missing {text:?}");
    }
    for label in ["R1", "R2", "R3", "R4", "R5", "R6"] {
        assert!(
            page.contains(&format!("<strong>{label}</strong>")),
            "missing {label}"
        );
    }
}

#[test]
fn every_threshold_appears_with_its_value() {
    let page = rules(Some(&thresholds(Some("2026-11-03"))));
    for (key, value) in [
        ("new.max_age_days", "180"),
        ("new.min_active_users", "2"),
        ("library_featured.min_active_users", "11"),
        ("library_featured.min_new_adopters", "9"),
        ("library_featured.min_former_glory", "0.5"),
        ("app_featured.min_signals", "2"),
        ("app_featured.binary_downloads", "884"),
        ("app_featured.homebrew_on_request_365d", "500"),
        ("app_featured.repology_families", "2"),
        ("app_featured.docker_pull_growth_90d", "10000"),
        ("dormancy.min_days", "365"),
        ("dormancy.dead_rot_signals", "2"),
        ("dormancy.dead_consecutive_checks", "2"),
        ("interval.patch_gt3_major_ge1", "700"),
        ("interval.patch_gt3", "450"),
        ("interval.patch_gt0", "300"),
        ("interval.major_ge1", "200"),
        ("interval.minor_gt3", "140"),
        ("interval.other", "80"),
        ("interval.days_per_release_cap", "50"),
        ("interval.downloads_per_month_double", "50000"),
        ("interval.active_users_double", "10"),
        ("rot.unanswered_days", "270"),
        ("rot.maintainer_silence_days", "365"),
        ("rot.advisory_grace_days", "60"),
        ("rot.lost_glory_below", "0.5"),
        ("rot.build_check_max_age_days", "45"),
        ("finished.min_former_glory", "0.8"),
        ("finished.min_new_adopters", "1"),
        ("finished.min_binary_downloads_per_release", "200"),
        ("fallback.max_age_days", "14"),
    ] {
        let cells =
            format!("<th scope=\"row\"><code>{key}</code></th><td class=\"number\">{value}</td>");
        assert!(page.contains(&cells), "missing {cells}");
    }
}

#[test]
fn uncalibrated_thresholds_say_tiers_are_not_published() {
    let page = rules(Some(&thresholds(None)));
    assert!(
        page.contains("Not calibrated — tiers are not published."),
        "{page}"
    );
    assert!(!page.contains("Calibrated on"), "{page}");
}

#[test]
fn without_thresholds_the_page_shows_no_values() {
    let page = rules(None);
    assert!(
        page.contains("data/thresholds.toml is absent. No tier is published."),
        "{page}"
    );
    assert!(!page.contains("<table>"), "{page}");
    assert!(page.contains("Stars never count toward a tier."), "{page}");
}
