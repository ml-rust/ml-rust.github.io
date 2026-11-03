//! Fixtures, scratch directories and the command runner of the `catalogue build` tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// Path of the fixture data directory named `case`.
pub fn fixture(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(case)
}

/// Path of the golden tier report.
pub fn golden() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("report.json")
}

/// A new scratch directory under `target/`.
pub fn scratch() -> TempDir {
    tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target"))
        .expect("scratch directory")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("dir");
    for item in fs::read_dir(from).expect("list") {
        let item = item.expect("item");
        let target = to.join(item.file_name());
        if item.path().is_dir() {
            copy_dir(&item.path(), &target);
        } else {
            fs::copy(item.path(), target).expect("copy");
        }
    }
}

/// The `valid` fixture with a `calibrated` date, in a new directory.
pub fn calibrated_data(scratch: &Path) -> PathBuf {
    let data = scratch.join("data");
    copy_dir(&fixture("valid"), &data);
    let path = data.join("thresholds.toml");
    let text = fs::read_to_string(&path).expect("thresholds");
    fs::write(&path, format!("calibrated = 2026-11-01\n{text}")).expect("write");
    data
}

/// The standard error of `output` as text.
pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Runs `catalogue build` with `args`, `--data` and `--out`.
pub fn site(args: &[&str], data: &Path, out: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_catalogue"));
    command.arg("build").args(args);
    command.arg("--data").arg(data).arg("--out").arg(out);
    command.output().expect("runs")
}
