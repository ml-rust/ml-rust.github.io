//! The CI guard and the scrubbed environment of `catalogue sandbox check`.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::error::SandboxCommandError;

/// The directory under `RUNNER_TEMP` that holds the probe projects.
const SANDBOX_DIR: &str = "catalogue-sandbox";

/// The variables a build keeps. Every other variable, secrets included, is cleared.
pub(super) const KEPT_VARS: [&str; 5] =
    ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "RUNNER_TEMP"];

/// The probe root, `<RUNNER_TEMP>/catalogue-sandbox`, when the run is inside CI.
///
/// `github_actions` and `runner_temp` are the values of `GITHUB_ACTIONS` and
/// `RUNNER_TEMP`. The check refuses unless `GITHUB_ACTIONS` is exactly `true`
/// and `RUNNER_TEMP` is an absolute path.
pub(super) fn sandbox_root(
    github_actions: Option<&OsStr>,
    runner_temp: Option<&OsStr>,
) -> Result<PathBuf, SandboxCommandError> {
    if github_actions != Some(OsStr::new("true")) {
        return Err(SandboxCommandError::NotInCi);
    }
    match runner_temp.map(Path::new) {
        Some(dir) if dir.is_absolute() => Ok(dir.join(SANDBOX_DIR)),
        _ => Err(SandboxCommandError::NoRunnerTemp),
    }
}

/// The [`KEPT_VARS`] that `get` finds set, in that order.
pub(super) fn kept_env(get: impl Fn(&str) -> Option<OsString>) -> Vec<(&'static str, OsString)> {
    KEPT_VARS
        .iter()
        .filter_map(|&name| get(name).map(|value| (name, value)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(value: &str) -> Option<&OsStr> {
        Some(OsStr::new(value))
    }

    #[test]
    fn the_check_refuses_outside_ci() {
        for value in [None, os("false"), os("TRUE"), os("1")] {
            let error = sandbox_root(value, os("/home/runner/work/_temp")).expect_err("refused");
            assert!(matches!(error, SandboxCommandError::NotInCi), "{error}");
        }
    }

    #[test]
    fn the_check_needs_an_absolute_runner_temp() {
        for value in [None, os(""), os("relative/temp")] {
            let error = sandbox_root(os("true"), value).expect_err("refused");
            assert!(
                matches!(error, SandboxCommandError::NoRunnerTemp),
                "{error}"
            );
        }
        let root = sandbox_root(os("true"), os("/home/runner/work/_temp")).expect("root");
        assert_eq!(
            root,
            PathBuf::from("/home/runner/work/_temp/catalogue-sandbox")
        );
    }

    #[test]
    fn only_the_kept_variables_pass() {
        let get = |name: &str| match name {
            "PATH" => Some(OsString::from("/usr/bin")),
            "CARGO_HOME" => Some(OsString::from("/c")),
            "GITHUB_TOKEN" => Some(OsString::from("secret")),
            _ => None,
        };
        let kept = kept_env(get);
        assert_eq!(
            kept,
            [
                ("PATH", OsString::from("/usr/bin")),
                ("CARGO_HOME", OsString::from("/c"))
            ]
        );
    }
}
