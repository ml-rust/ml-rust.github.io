//! Turns the output of one `cargo check` into a result with a fixed-text detail.

use super::message::{CompilerError, compiler_error};
use super::record::CheckResult;

/// How the `cargo check` process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Cargo exits 0.
    Success,
    /// Cargo exits with another code or a signal.
    Failure,
    /// The runner stops cargo at the time limit.
    TimedOut,
}

/// The result of one check and its fixed-text detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    /// The outcome.
    pub result: CheckResult,
    /// A fixed-text reason. It holds no compiler or build script output, at most an error code.
    pub detail: String,
}

impl Classified {
    fn new(result: CheckResult, detail: impl Into<String>) -> Self {
        Self {
            result,
            detail: detail.into(),
        }
    }

    fn inconclusive(detail: &str) -> Self {
        Self::new(CheckResult::Inconclusive, detail)
    }
}

/// Markers in cargo's stderr that name why a build ended without a compiler error.
///
/// Only the fixed detail beside a marker reaches the result. The stderr text itself never does.
const STDERR_MARKERS: [(&str, &str); 8] = [
    (
        "failed to run custom build command",
        "a build script fails to run",
    ),
    ("failed to select a version", "dependency resolution fails"),
    ("no matching package named", "dependency resolution fails"),
    ("failed to download", "dependency resolution fails"),
    ("failed to load source", "dependency resolution fails"),
    ("linking with", "linking or a system library fails"),
    (
        "could not find native static library",
        "linking or a system library fails",
    ),
    (
        "unable to find library",
        "linking or a system library fails",
    ),
];

/// Classifies one `cargo check --message-format=json` of crate `name` at `version`.
///
/// `stdout` holds cargo's JSON lines and `stderr` its text. The rules, in order:
///
/// - A timeout is `inconclusive`.
/// - A `compiler-message` at level `error` whose package id is `name` at `version`,
///   outside its build script, is `fails`. The detail keeps the first rustc error code.
/// - Exit 0 is `passes`.
/// - Anything else is `inconclusive`: a build script error, a dependency's compile
///   error, a build script run, dependency resolution, the linker, or no known cause.
pub fn classify(name: &str, version: &str, exit: Exit, stdout: &str, stderr: &str) -> Classified {
    if exit == Exit::TimedOut {
        return Classified::inconclusive("cargo check timed out");
    }
    let errors: Vec<CompilerError> = stdout.lines().filter_map(compiler_error).collect();
    let mut own = errors
        .iter()
        .filter(|error| error.is(name, version) && !error.build_script)
        .peekable();
    if own.peek().is_some() {
        let detail = match own.find_map(|error| error.code.as_deref()) {
            Some(code) => format!("the crate fails to compile: {code}"),
            None => "the crate fails to compile".to_owned(),
        };
        return Classified::new(CheckResult::Fails, detail);
    }
    if exit == Exit::Success {
        return Classified::new(CheckResult::Passes, "cargo check passes");
    }
    if errors.iter().any(|error| error.build_script) {
        return Classified::inconclusive("a build script fails to compile");
    }
    if !errors.is_empty() {
        return Classified::inconclusive("a dependency fails to compile");
    }
    let marked = STDERR_MARKERS
        .iter()
        .find(|(marker, _)| stderr.contains(marker));
    match marked {
        Some((_, detail)) => Classified::inconclusive(detail),
        None => Classified::inconclusive("cargo check fails without a compiler error in the crate"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = "registry+https://github.com/rust-lang/crates.io-index";

    /// A `compiler-message` line in the shape cargo writes.
    fn message(package: &str, version: &str, kind: &str, level: &str, id: &str) -> String {
        let code = if id.is_empty() {
            "null".to_owned()
        } else {
            format!(r#"{{"code":"{id}","explanation":"An undefined method was used."}}"#)
        };
        format!(
            r#"{{"reason":"compiler-message","package_id":"{INDEX}#{package}@{version}","manifest_path":"/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/{package}-{version}/Cargo.toml","target":{{"kind":["{kind}"],"crate_types":["{kind}"],"name":"{package}","src_path":"/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/{package}-{version}/src/lib.rs","edition":"2021","doc":true,"doctest":true,"test":true}},"message":{{"rendered":"error[{id}]: no method named `foo` found\n","$message_type":"diagnostic","children":[],"code":{code},"level":"{level}","message":"no method named `foo` found for struct `Bar` in the current scope","spans":[]}}}}"#
        )
    }

    fn artifact(package: &str, version: &str) -> String {
        format!(
            r#"{{"reason":"compiler-artifact","package_id":"{INDEX}#{package}@{version}","manifest_path":"/x/Cargo.toml","target":{{"kind":["lib"],"crate_types":["lib"],"name":"{package}","src_path":"/x/src/lib.rs","edition":"2021","doc":true,"doctest":true,"test":true}},"profile":{{"opt_level":"0","debuginfo":2,"debug_assertions":true,"overflow_checks":true,"test":false}},"features":["default"],"filenames":["/x/lib{package}.rmeta"],"executable":null,"fresh":false}}"#
        )
    }

    const FINISHED_OK: &str = r#"{"reason":"build-finished","success":true}"#;
    const FINISHED_ERR: &str = r#"{"reason":"build-finished","success":false}"#;

    fn run(exit: Exit, lines: &[String], stderr: &str) -> Classified {
        classify("ndarray-ext", "0.4.1", exit, &lines.join("\n"), stderr)
    }

    #[test]
    fn a_clean_check_passes() {
        let lines = [
            artifact("libc", "0.2.159"),
            message("ndarray-ext", "0.4.1", "lib", "warning", ""),
            artifact("ndarray-ext", "0.4.1"),
            FINISHED_OK.to_owned(),
        ];
        let found = run(Exit::Success, &lines, "");
        assert_eq!(found.result, CheckResult::Passes);
        assert_eq!(found.detail, "cargo check passes");
    }

    #[test]
    fn an_error_in_the_crate_fails_with_its_code() {
        let lines = [
            artifact("libc", "0.2.159"),
            message("ndarray_ext", "0.4.1", "lib", "error", "E0599"),
            message("ndarray_ext", "0.4.1", "lib", "error", ""),
            FINISHED_ERR.to_owned(),
        ];
        let found = run(
            Exit::Failure,
            &lines,
            "error: could not compile `ndarray-ext`",
        );
        assert_eq!(found.result, CheckResult::Fails);
        assert_eq!(found.detail, "the crate fails to compile: E0599");
        let uncoded = [message("ndarray-ext", "0.4.1", "lib", "error", "")];
        assert_eq!(
            run(Exit::Failure, &uncoded, "").detail,
            "the crate fails to compile"
        );
    }

    #[test]
    fn an_error_in_another_version_of_the_crate_is_a_dependency_error() {
        let lines = [message("ndarray-ext", "0.3.0", "lib", "error", "E0433")];
        let found = run(Exit::Failure, &lines, "");
        assert_eq!(found.result, CheckResult::Inconclusive);
        assert_eq!(found.detail, "a dependency fails to compile");
    }

    #[test]
    fn an_error_in_a_dependency_is_inconclusive() {
        let lines = [
            message("old-dep", "0.1.9", "lib", "error", "E0277"),
            FINISHED_ERR.to_owned(),
        ];
        let found = run(Exit::Failure, &lines, "error: could not compile `old-dep`");
        assert_eq!(found.result, CheckResult::Inconclusive);
        assert_eq!(found.detail, "a dependency fails to compile");
    }

    #[test]
    fn a_build_script_error_is_inconclusive_even_in_the_crate() {
        let lines = [message(
            "ndarray-ext",
            "0.4.1",
            "custom-build",
            "error",
            "E0425",
        )];
        let found = run(Exit::Failure, &lines, "");
        assert_eq!(found.result, CheckResult::Inconclusive);
        assert_eq!(found.detail, "a build script fails to compile");
    }

    #[test]
    fn a_failed_build_script_run_is_inconclusive() {
        let stderr = "error: failed to run custom build command for `openblas-src v0.10.9`\n\
                      Caused by: process didn't exit successfully";
        let found = run(Exit::Failure, &[FINISHED_ERR.to_owned()], stderr);
        assert_eq!(found.result, CheckResult::Inconclusive);
        assert_eq!(found.detail, "a build script fails to run");
    }

    #[test]
    fn resolution_and_linker_errors_are_inconclusive() {
        let resolution = "error: failed to select a version for the requirement `tch = \"=0.1.0\"`";
        assert_eq!(
            run(Exit::Failure, &[], resolution).detail,
            "dependency resolution fails"
        );
        let linker = "error: linking with `cc` failed: exit status: 1";
        assert_eq!(
            run(Exit::Failure, &[], linker).detail,
            "linking or a system library fails"
        );
        let unknown = run(Exit::Failure, &[], "something else");
        assert_eq!(unknown.result, CheckResult::Inconclusive);
        assert_eq!(
            unknown.detail,
            "cargo check fails without a compiler error in the crate"
        );
    }

    #[test]
    fn a_timeout_is_inconclusive_whatever_the_messages() {
        let lines = [message("ndarray-ext", "0.4.1", "lib", "error", "E0599")];
        let found = run(Exit::TimedOut, &lines, "");
        assert_eq!(found.result, CheckResult::Inconclusive);
        assert_eq!(found.detail, "cargo check timed out");
    }

    #[test]
    fn the_detail_never_holds_compiler_text() {
        let lines = [message("ndarray-ext", "0.4.1", "lib", "error", "E0599")];
        let found = run(Exit::Failure, &lines, "");
        assert!(!found.detail.contains("no method"), "{}", found.detail);
    }
}
