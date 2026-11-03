//! `catalogue sandbox` and the R6 results that `catalogue tiers` reads.

#[path = "common/tiers_dump.rs"]
mod tiers_dump;

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use catalogue::{Network, load_catalogue, sandbox_list, tiers_report};
use catalogue_core::{Catalogue, Date, EntryKind, Tier};
use catalogue_signals::history::{History, Row, Snapshot};
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::{BuildChecks, CheckItem, CheckRecord, CheckResult};
use tiers_dump::{fixture, snapshot};

fn catalogue() -> Catalogue {
    load_catalogue(&fixture("valid"))
        .expect("fixture reads")
        .expect("fixture is valid")
}

fn date(day: i8) -> Date {
    Date::new(2026, 11, day).expect("date")
}

fn row(dormant: Option<bool>) -> Row {
    Row {
        kind: EntryKind::Library,
        tier: Tier::Listed,
        dead_now: false,
        dormant,
        active_users: None,
        docker_pulls: None,
    }
}

fn history(rows: &[(i8, &str, Option<bool>)]) -> History {
    History::new(
        rows.iter()
            .map(|(day, slug, dormant)| Snapshot {
                date: date(*day),
                rows: [((*slug).to_owned(), row(*dormant))].into_iter().collect(),
            })
            .collect(),
    )
}

/// The note for `beta`, which a `finished` statement lists and the dump lacks.
const BETA_NOTE: &str = "beta: crate beta-crate has no unyanked release in the dump, skipped";

#[test]
fn the_newest_snapshot_decides_which_entries_are_dormant() {
    let dormant = history(&[(1, "alpha", Some(false)), (2, "alpha", Some(true))]);
    let found = sandbox_list(&catalogue(), &dormant, &snapshot(), date(3));
    assert_eq!(
        found.items,
        [CheckItem {
            entry: "alpha".to_owned(),
            name: "alpha-crate".to_owned(),
            version: "1.3.0".to_owned(),
        }]
    );
    assert_eq!(found.notes, [BETA_NOTE]);
    let woke = history(&[(1, "alpha", Some(true)), (2, "alpha", Some(false))]);
    let found = sandbox_list(&catalogue(), &woke, &snapshot(), date(3));
    assert!(found.items.is_empty(), "{:?}", found.items);
}

#[test]
fn unknown_dormancy_and_an_empty_history_list_no_crate() {
    let unknown = history(&[(2, "alpha", None)]);
    let found = sandbox_list(&catalogue(), &unknown, &snapshot(), date(3));
    assert!(found.items.is_empty(), "{:?}", found.items);
    let empty = sandbox_list(&catalogue(), &History::default(), &snapshot(), date(3));
    assert!(empty.items.is_empty(), "{:?}", empty.items);
    assert_eq!(empty.notes, [BETA_NOTE]);
}

fn record(version: &str, result: CheckResult) -> CheckRecord {
    CheckRecord {
        name: "alpha-crate".to_owned(),
        version: version.to_owned(),
        toolchain: "rustc 1.95.0 (abc 2026-10-01)".to_owned(),
        checked: date(1),
        result,
        detail: "the crate fails to compile: E0599".to_owned(),
    }
}

fn alpha_block(checks: &BuildChecks) -> String {
    let report = tiers_report(
        &catalogue(),
        &snapshot(),
        &Advisories::default(),
        checks,
        Network::Offline,
        None,
        date(3),
    )
    .expect("report")
    .text;
    report
        .split("\n\n")
        .find(|block| block.starts_with("slug: alpha\n"))
        .unwrap_or_else(|| panic!("no block for alpha in:\n{report}"))
        .to_owned()
}

const R6_REASON: &str = "\n  rot: latest release fails `cargo check` on rustc 1.95.0 (abc 2026-10-01), checked 2026-11-01\n";

fn unknown_line(block: &str) -> &str {
    block
        .lines()
        .find(|line| line.starts_with("unknown: "))
        .unwrap_or_default()
}

#[test]
fn a_failing_result_for_the_latest_release_is_rot_signal_r6() {
    let fails = alpha_block(&BuildChecks::new(vec![record("1.3.0", CheckResult::Fails)]));
    assert!(fails.contains(R6_REASON), "{fails}");
    assert!(!unknown_line(&fails).contains("build_fails"), "{fails}");
    let passes = alpha_block(&BuildChecks::new(vec![record(
        "1.3.0",
        CheckResult::Passes,
    )]));
    assert!(!passes.contains("cargo check"), "{passes}");
    assert!(!unknown_line(&passes).contains("build_fails"), "{passes}");
}

/// The fixture entry is active, so an unknown build check is not listed: rot
/// counts only for a dormant entry.
#[test]
fn a_result_for_another_version_or_an_inconclusive_one_gives_no_finding() {
    for checks in [
        BuildChecks::new(vec![record("1.2.0", CheckResult::Fails)]),
        BuildChecks::new(vec![record("1.3.0", CheckResult::Inconclusive)]),
        BuildChecks::default(),
    ] {
        let block = alpha_block(&checks);
        assert!(!block.contains("cargo check"), "{block}");
        assert!(!unknown_line(&block).contains("build_fails"), "{block}");
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

#[test]
fn check_refuses_to_run_outside_ci() {
    let dir = scratch("sandbox-refuse");
    let list = dir.join("list.jsonl");
    fs::write(
        &list,
        "{\"entry\":\"alpha\",\"crate\":\"alpha-crate\",\"version\":\"1.3.0\"}\n",
    )
    .expect("list");
    let out = dir.join("build-checks.jsonl");
    let run = |github_actions: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_catalogue"));
        command
            .args(["sandbox", "check", "--list"])
            .arg(&list)
            .arg("--out")
            .arg(&out)
            .env_remove("GITHUB_ACTIONS")
            .env_remove("RUNNER_TEMP");
        if let Some(value) = github_actions {
            command.env("GITHUB_ACTIONS", value);
        }
        command.output().expect("catalogue runs")
    };
    for value in [None, Some("false")] {
        let output = run(value);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(
            stderr.contains("runs third-party build scripts"),
            "{stderr}"
        );
        assert!(stderr.contains("GITHUB_ACTIONS is not `true`"), "{stderr}");
    }
    let output = run(Some("true"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("RUNNER_TEMP is unset"), "{stderr}");
    assert!(!out.exists(), "no result file is written");
}
