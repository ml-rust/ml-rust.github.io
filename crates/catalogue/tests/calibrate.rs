#[path = "common/calibrate_dump.rs"]
mod calibrate_dump;

use calibrate_dump::{rules, rules_without_match, snapshot};
use catalogue::build_crates_report;

const HEADER: &str = "calibration: crates.io discovery candidates with at least one active user (current >= 1), dump 2026-11-02";

#[test]
fn the_report_states_the_population_and_the_dump_date() {
    let text = build_crates_report(&snapshot(), &rules(), 90.0);
    assert!(text.starts_with(&format!("{HEADER}\n\n")), "{text}");
    let signals: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("signal: "))
        .collect();
    assert_eq!(
        signals,
        [
            "active_users",
            "new_adopters",
            "former_glory",
            "release_count"
        ]
    );
}

#[test]
fn only_candidates_with_an_active_user_are_counted() {
    let text = build_crates_report(&snapshot(), &rules(), 90.0);
    assert_eq!(
        text.matches("population: 2\n").count(),
        4,
        "core-lib and other count, lonely has no user:\n{text}"
    );
}

#[test]
fn active_users_and_release_counts_follow_nearest_rank() {
    let text = build_crates_report(&snapshot(), &rules(), 90.0);
    let active = "signal: active_users\npopulation: 2\n\
        p50: 1.00\np75: 3.00\np90: 3.00\np95: 3.00\np99: 3.00\nat p90: 3.00\n\n";
    let releases = "signal: release_count\npopulation: 2\n\
        p50: 1.00\np75: 2.00\np90: 2.00\np95: 2.00\np99: 2.00\nat p90: 2.00";
    assert!(text.contains(active), "{text}");
    assert!(text.ends_with(releases), "{text}");
}

#[test]
fn a_custom_percentile_adds_one_line_per_block() {
    let text = build_crates_report(&snapshot(), &rules(), 60.0);
    assert_eq!(text.matches("\nat p60: ").count(), 4);
    assert!(
        text.contains("p99: 3.00\nat p60: 3.00"),
        "ceil(0.6 * 2) = 2 reads the larger sample:\n{text}"
    );
}

#[test]
fn a_population_with_no_candidate_prints_dashes() {
    let text = build_crates_report(&snapshot(), &rules_without_match(), 90.0);
    assert_eq!(text.matches("population: 0\np50: -\n").count(), 4, "{text}");
}
