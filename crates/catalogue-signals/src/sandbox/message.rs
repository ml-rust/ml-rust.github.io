//! The parts of a `cargo check --message-format=json` line the classification reads.

use catalogue_core::canonical_crate_name;
use serde::Deserialize;

/// One JSON line from cargo. Fields the classification does not read are ignored.
#[derive(Deserialize)]
struct Line {
    reason: String,
    package_id: Option<String>,
    target: Option<Target>,
    message: Option<Diagnostic>,
}

#[derive(Deserialize)]
struct Target {
    kind: Vec<String>,
}

#[derive(Deserialize)]
struct Diagnostic {
    level: String,
    code: Option<Code>,
}

#[derive(Deserialize)]
struct Code {
    code: String,
}

/// A compiler error that cargo reports for one package target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompilerError {
    /// The package name from the package id.
    pub name: String,
    /// The package version from the package id.
    pub version: String,
    /// Whether the error is in the package's build script.
    pub build_script: bool,
    /// The rustc error code, when it has the `E` and four digits form.
    pub code: Option<String>,
}

impl CompilerError {
    /// Whether the error belongs to crate `name` at `version`.
    ///
    /// Names match in any spelling crates.io treats as equal.
    pub fn is(&self, name: &str, version: &str) -> bool {
        self.version == version && canonical_crate_name(&self.name) == canonical_crate_name(name)
    }
}

/// The compiler error that `text` reports, when it is a `compiler-message` line at level `error`.
///
/// A line that is not JSON, another reason, another level, or a package id
/// without a name and version gives `None`.
pub(super) fn compiler_error(text: &str) -> Option<CompilerError> {
    let line: Line = serde_json::from_str(text).ok()?;
    if line.reason != "compiler-message" {
        return None;
    }
    let message = line.message?;
    if message.level != "error" {
        return None;
    }
    let (name, version) = package(line.package_id.as_deref()?)?;
    let build_script = line
        .target
        .is_some_and(|target| target.kind.iter().any(|kind| kind == "custom-build"));
    let code = message
        .code
        .map(|code| code.code)
        .filter(|code| is_error_code(code));
    Some(CompilerError {
        name,
        version,
        build_script,
        code,
    })
}

/// The name and version in a cargo package id.
///
/// Reads both forms: `registry+https://github.com/rust-lang/crates.io-index#foo@1.2.3`,
/// with the name taken from the URL path when the fragment holds only a version,
/// and the older `foo 1.2.3 (registry+https://github.com/rust-lang/crates.io-index)`.
fn package(id: &str) -> Option<(String, String)> {
    if id.contains(' ') {
        let mut parts = id.split_whitespace();
        let name = parts.next()?;
        let version = parts.next()?;
        return Some((name.to_owned(), version.to_owned()));
    }
    let (source, fragment) = id.rsplit_once('#')?;
    match fragment.split_once('@') {
        Some((name, version)) => Some((name.to_owned(), version.to_owned())),
        None => {
            let name = source.trim_end_matches('/').rsplit('/').next()?;
            Some((name.to_owned(), fragment.to_owned()))
        }
    }
}

/// Whether `code` is a rustc error code: `E` and four digits.
fn is_error_code(code: &str) -> bool {
    code.len() == 5
        && code.starts_with('E')
        && code.bytes().skip(1).all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_package_id_forms_give_a_name_and_version() {
        let new = "registry+https://github.com/rust-lang/crates.io-index#serde_json@1.0.128";
        assert_eq!(
            package(new),
            Some(("serde_json".to_owned(), "1.0.128".to_owned()))
        );
        let old = "serde_json 1.0.128 (registry+https://github.com/rust-lang/crates.io-index)";
        assert_eq!(
            package(old),
            Some(("serde_json".to_owned(), "1.0.128".to_owned()))
        );
        let path = "path+file:///runner/_temp/sandbox/probe#0.0.0";
        assert_eq!(
            package(path),
            Some(("probe".to_owned(), "0.0.0".to_owned()))
        );
        assert_eq!(package("garbage"), None);
    }

    #[test]
    fn only_rustc_error_codes_are_kept() {
        assert!(is_error_code("E0599"));
        assert!(!is_error_code("E059"));
        assert!(!is_error_code("unused_imports"));
        assert!(!is_error_code("EABCD"));
    }

    #[test]
    fn warnings_and_other_reasons_are_not_errors() {
        let warning = r#"{"reason":"compiler-message","package_id":"registry+https://github.com/rust-lang/crates.io-index#a@1.0.0","target":{"kind":["lib"]},"message":{"level":"warning","code":null}}"#;
        assert_eq!(compiler_error(warning), None);
        let artifact = r#"{"reason":"compiler-artifact","package_id":"registry+https://github.com/rust-lang/crates.io-index#a@1.0.0","target":{"kind":["lib"]}}"#;
        assert_eq!(compiler_error(artifact), None);
        assert_eq!(compiler_error("not json"), None);
    }

    #[test]
    fn spellings_crates_io_treats_as_equal_match() {
        let error = CompilerError {
            name: "foo_bar".to_owned(),
            version: "1.0.0".to_owned(),
            build_script: false,
            code: None,
        };
        assert!(error.is("Foo-Bar", "1.0.0"));
        assert!(!error.is("foo-bar", "1.0.1"));
    }
}
