#[path = "common/build_site.rs"]
mod build_site;

use std::fs;
use std::process::Command;

use build_site::{calibrated_data, fixture, golden, scratch, site, stderr};

#[test]
fn no_tiers_writes_every_page_and_the_search_files() {
    let dir = scratch();
    let out = dir.path().join("site");
    let data = fixture("valid");
    let output = Command::new(env!("CARGO_BIN_EXE_catalogue"))
        .args(["build", "--no-tiers", "--data"])
        .arg(&data)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("runs");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    for page in [
        "index.html",
        "rules/index.html",
        "submit/index.html",
        "404.html",
        "topics/index.html",
        "changes/index.html",
        "search/index.html",
    ] {
        assert!(out.join(page).is_file(), "{page}");
    }
    assert!(
        fs::read_dir(out.join("projects"))
            .expect("projects")
            .count()
            > 0
    );
    assert!(fs::read_dir(out.join("search")).expect("search").count() > 1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("ok: "), "{stdout}");
    assert!(stdout.contains("site"), "{stdout}");
}

#[test]
fn an_uncalibrated_fixture_refuses_a_report() {
    let dir = scratch();
    let out = dir.path().join("site");
    let report = golden();
    let mut command = Command::new(env!("CARGO_BIN_EXE_catalogue"));
    command
        .args(["build", "--report"])
        .arg(&report)
        .arg("--data")
        .arg(fixture("valid"))
        .arg("--out")
        .arg(&out);
    let output = command.output().expect("runs");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("calibrated"),
        "{}",
        stderr(&output)
    );
    assert!(!out.exists());
}

#[test]
fn a_calibrated_fixture_builds_with_the_report() {
    let dir = scratch();
    let data = calibrated_data(dir.path());
    let out = dir.path().join("site");
    let mut command = Command::new(env!("CARGO_BIN_EXE_catalogue"));
    command
        .args(["build", "--report"])
        .arg(golden())
        .arg("--data")
        .arg(&data)
        .arg("--out")
        .arg(&out);
    let output = command.output().expect("runs");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(out.join("index.html").is_file());
}

#[test]
fn a_second_build_replaces_the_directory() {
    let dir = scratch();
    let out = dir.path().join("site");
    fs::create_dir_all(&out).expect("dir");
    fs::write(out.join("stale.html"), "old").expect("stale");
    for _ in 0..2 {
        let output = site(&["--no-tiers"], &fixture("valid"), &out);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert!(!out.join("stale.html").exists());
        assert!(out.join("index.html").is_file());
        fs::write(out.join("stale.html"), "old").expect("stale");
    }
    assert!(!dir.path().join(".site.staging").exists());
    assert!(!dir.path().join(".site.old").exists());
}

#[test]
fn bad_report_json_names_the_file_and_exits_one() {
    let dir = scratch();
    let report = dir.path().join("report.json");
    fs::write(&report, "{not json").expect("report");
    let out = dir.path().join("site");
    let mut command = Command::new(env!("CARGO_BIN_EXE_catalogue"));
    command
        .args(["build", "--report"])
        .arg(&report)
        .arg("--data")
        .arg(fixture("valid"))
        .arg("--out")
        .arg(&out);
    let output = command.output().expect("runs");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("report.json"),
        "{}",
        stderr(&output)
    );
    assert!(!out.exists());
}

#[test]
fn invalid_data_prints_diagnostics_and_exits_one() {
    let dir = scratch();
    let out = dir.path().join("site");
    let output = site(&["--no-tiers"], &fixture("duplicate-crate"), &out);
    assert_eq!(output.status.code(), Some(1));
    assert!(!stderr(&output).is_empty());
    assert!(!out.exists());
}

#[test]
fn a_missing_report_file_exits_two() {
    let dir = scratch();
    let out = dir.path().join("site");
    let missing = dir.path().join("absent.json");
    let mut command = Command::new(env!("CARGO_BIN_EXE_catalogue"));
    command
        .args(["build", "--report"])
        .arg(&missing)
        .arg("--data")
        .arg(fixture("valid"))
        .arg("--out")
        .arg(&out);
    let output = command.output().expect("runs");
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("absent.json"));
}

#[test]
fn an_out_directory_that_holds_the_data_is_refused() {
    let dir = scratch();
    let data = calibrated_data(dir.path());
    let output = site(&["--no-tiers"], &data, dir.path());
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(data.join("thresholds.toml").is_file());
}

#[test]
fn a_flag_is_required() {
    let output = Command::new(env!("CARGO_BIN_EXE_catalogue"))
        .arg("build")
        .output()
        .expect("runs");
    assert_eq!(output.status.code(), Some(2));
}
