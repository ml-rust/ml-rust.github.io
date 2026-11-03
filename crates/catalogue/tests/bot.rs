//! `catalogue bot check` run as a process over the `valid` data fixture.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Path of the fixture data directory named `case`.
fn fixture(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(case)
}

/// The issue body fixture `name` of the `catalogue-bot` crate.
fn body(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../catalogue-bot/tests/fixtures")
        .join(name)
}

fn check(label: &str, body: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_catalogue"))
        .args(["bot", "check", "--label", label, "--body"])
        .arg(body)
        .args(["--issue", "12", "--author", "dave", "--today", "2026-11-03"])
        .arg("--data")
        .arg(fixture("valid"))
        .output()
        .expect("catalogue runs")
}

#[test]
fn a_valid_submission_prints_the_report_and_exits_zero() {
    let output = check("submit", &body("submit.md"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{stdout}");
    assert!(stdout.starts_with("## Catalogue check\n"), "{stdout}");
    assert!(
        stdout.contains("Path: `data/entries/gizmo.toml`."),
        "{stdout}"
    );
    assert!(
        stdout.contains("The new file passes every data rule."),
        "{stdout}"
    );
    assert!(
        stdout.contains("Eligible approvers: `bob`, `carol`."),
        "{stdout}"
    );
}

#[test]
fn a_blocking_report_exits_one() {
    let output = check("submit", &body("forged-heading.md"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("**Result: blocking.**"), "{stdout}");
}

#[test]
fn a_missing_body_exits_two() {
    let output = check("submit", &body("no-such-body.md"));
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot read issue body"), "{stderr}");
}
