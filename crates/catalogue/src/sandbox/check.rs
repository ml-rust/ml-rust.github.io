//! The `catalogue sandbox check` loop: one probe build per list item.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use catalogue_core::Date;
use catalogue_signals::sandbox::{CheckItem, CheckRecord, CheckResult, classify};

use super::error::SandboxCommandError;
use super::files::{ResultFile, read_file};
use super::guard::{kept_env, sandbox_root};
use super::probe;
use super::run::cargo_check;
use crate::clock::today_utc;

/// Seconds in a minute.
const MINUTE: u64 = 60;

/// The longest toolchain label kept.
const MAX_LABEL_LEN: usize = 200;

/// The inputs of one `catalogue sandbox check` run.
pub(super) struct CheckRun<'a> {
    /// The list file of `catalogue sandbox list`.
    pub list: &'a Path,
    /// The result file to write.
    pub out: &'a Path,
    /// The toolchain label. `None` reads `rustc --version`.
    pub toolchain_label: Option<&'a str>,
    /// The time limit of each build, in minutes.
    pub timeout_minutes: u16,
}

/// The result counts of one run.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Counts {
    pub passes: usize,
    pub fails: usize,
    pub inconclusive: usize,
}

/// Runs the check over every list item and writes one result line per item.
///
/// `env` reads the process environment. The run refuses before it reads any
/// file unless [`sandbox_root`] accepts `GITHUB_ACTIONS` and `RUNNER_TEMP`.
/// Every item is checked before the first build starts.
pub(super) fn check(
    run: &CheckRun<'_>,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<Counts, SandboxCommandError> {
    let root = sandbox_root(
        env("GITHUB_ACTIONS").as_deref(),
        env("RUNNER_TEMP").as_deref(),
    )?;
    if run.timeout_minutes == 0 {
        return Err(SandboxCommandError::ZeroTimeout);
    }
    let timeout = Duration::from_secs(u64::from(run.timeout_minutes).saturating_mul(MINUTE));
    let items: Vec<CheckItem> = read_file(run.list)?;
    for (index, item) in items.iter().enumerate() {
        probe::validate(item).map_err(|reason| SandboxCommandError::InvalidItem {
            path: run.list.to_path_buf(),
            line: index.saturating_add(1),
            reason,
        })?;
    }
    let kept = kept_env(env);
    fs::create_dir_all(&root).map_err(write_error(&root))?;
    let toolchain = match run.toolchain_label {
        Some(label) => one_line(label),
        None => rustc_version(&root, &kept)?,
    };
    let mut out = ResultFile::create(run.out)?;
    let mut counts = Counts::default();
    for item in &items {
        let dir = root.join("probe");
        probe::create(&dir, item).map_err(write_error(&dir))?;
        let output = cargo_check(&dir, &kept, timeout);
        let removed = fs::remove_dir_all(&dir);
        let output = output?;
        if let Err(error) = removed {
            eprintln!(
                "warning: cannot remove {}: {error}. The next probe replaces it",
                dir.display()
            );
        }
        let found = classify(
            &item.name,
            &item.version,
            output.exit,
            &output.stdout,
            &output.stderr,
        );
        match found.result {
            CheckResult::Passes => counts.passes = counts.passes.saturating_add(1),
            CheckResult::Fails => counts.fails = counts.fails.saturating_add(1),
            CheckResult::Inconclusive => {
                counts.inconclusive = counts.inconclusive.saturating_add(1);
            }
        }
        eprintln!(
            "{} {}: {} ({})",
            item.name,
            item.version,
            found.result.as_str(),
            found.detail
        );
        out.append(&CheckRecord {
            name: item.name.clone(),
            version: item.version.clone(),
            toolchain: toolchain.clone(),
            checked: Date::from(today_utc()),
            result: found.result,
            detail: found.detail,
        })?;
    }
    Ok(counts)
}

/// The output of `rustc --version`, run in `dir` with the scrubbed environment.
fn rustc_version(
    dir: &Path,
    env: &[(&'static str, OsString)],
) -> Result<String, SandboxCommandError> {
    let spawn = |source| SandboxCommandError::Spawn {
        program: "rustc",
        source,
    };
    let output = Command::new("rustc")
        .arg("--version")
        .current_dir(dir)
        .env_clear()
        .envs(env.iter().map(|(name, value)| (*name, value)))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(spawn)?;
    if !output.status.success() {
        return Err(spawn(io::Error::other(format!(
            "rustc --version exits with {}",
            output.status
        ))));
    }
    Ok(one_line(&String::from_utf8_lossy(&output.stdout)))
}

/// The first line of `text`, trimmed and cut to [`MAX_LABEL_LEN`] characters.
fn one_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .trim()
        .chars()
        .take(MAX_LABEL_LEN)
        .collect()
}

/// Wraps an IO error as a write error for `path`.
fn write_error(path: &Path) -> impl FnOnce(io::Error) -> SandboxCommandError {
    let path: PathBuf = path.to_path_buf();
    move |source| SandboxCommandError::Write { path, source }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run<'a>(list: &'a Path, out: &'a Path) -> CheckRun<'a> {
        CheckRun {
            list,
            out,
            toolchain_label: Some("rustc 1.95.0"),
            timeout_minutes: 15,
        }
    }

    #[test]
    fn the_run_refuses_outside_ci_before_it_reads_the_list() {
        let missing = Path::new("/nonexistent/list.jsonl");
        let error = check(&run(missing, missing), |_| None).expect_err("refused");
        assert!(matches!(error, SandboxCommandError::NotInCi), "{error}");
        let ci = |name: &str| (name == "GITHUB_ACTIONS").then(|| OsString::from("true"));
        let error = check(&run(missing, missing), ci).expect_err("refused");
        assert!(
            matches!(error, SandboxCommandError::NoRunnerTemp),
            "{error}"
        );
    }

    #[test]
    fn a_zero_timeout_is_refused() {
        let missing = Path::new("/nonexistent/list.jsonl");
        let env = |name: &str| match name {
            "GITHUB_ACTIONS" => Some(OsString::from("true")),
            "RUNNER_TEMP" => Some(OsString::from("/nonexistent/runner")),
            _ => None,
        };
        let zero = CheckRun {
            timeout_minutes: 0,
            ..run(missing, missing)
        };
        let error = check(&zero, env).expect_err("refused");
        assert!(matches!(error, SandboxCommandError::ZeroTimeout), "{error}");
    }

    #[test]
    fn a_label_keeps_one_trimmed_line() {
        assert_eq!(
            one_line("  rustc 1.95.0 (abc 2026-10-01)\nextra"),
            "rustc 1.95.0 (abc 2026-10-01)"
        );
        assert_eq!(one_line(""), "");
        assert_eq!(one_line(&"x".repeat(500)).len(), MAX_LABEL_LEN);
    }
}
