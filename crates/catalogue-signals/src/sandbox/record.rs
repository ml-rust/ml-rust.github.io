//! The JSON lines the sandbox check reads and writes.

use catalogue_core::Date;
use serde::{Deserialize, Serialize};

/// One crate release the sandbox check builds: a line of the list file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckItem {
    /// The slug of the entry that lists the crate.
    pub entry: String,
    /// The crate name as crates.io stores it.
    #[serde(rename = "crate")]
    pub name: String,
    /// The release to build: the crate's latest version.
    pub version: String,
}

/// The outcome of one `cargo check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckResult {
    /// Cargo exits 0.
    Passes,
    /// The crate under test has a compile error of its own.
    Fails,
    /// Anything else: a dependency, a build script, the linker, resolution or a timeout.
    Inconclusive,
}

impl CheckResult {
    /// The lowercase name used in the result file.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passes => "passes",
            Self::Fails => "fails",
            Self::Inconclusive => "inconclusive",
        }
    }
}

/// One result of the sandbox check: a line of the result file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRecord {
    /// The crate name as crates.io stores it.
    #[serde(rename = "crate")]
    pub name: String,
    /// The release built.
    pub version: String,
    /// The toolchain label, such as the output of `rustc --version`.
    pub toolchain: String,
    /// The UTC date of the check, written as `YYYY-MM-DD`.
    #[serde(serialize_with = "catalogue_core::date_text::plain")]
    pub checked: Date,
    /// The outcome.
    pub result: CheckResult,
    /// A fixed-text reason. It never holds compiler output, at most an error code such as `E0599`.
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_round_trips_with_a_plain_date() {
        let record = CheckRecord {
            name: "alpha".to_owned(),
            version: "1.2.3".to_owned(),
            toolchain: "rustc 1.95.0 (abc 2026-10-01)".to_owned(),
            checked: Date::new(2026, 11, 2).expect("date"),
            result: CheckResult::Fails,
            detail: "the crate fails to compile: E0599".to_owned(),
        };
        let text = serde_json::to_string(&record).expect("serialize");
        assert_eq!(
            text,
            r#"{"crate":"alpha","version":"1.2.3","toolchain":"rustc 1.95.0 (abc 2026-10-01)","checked":"2026-11-02","result":"fails","detail":"the crate fails to compile: E0599"}"#
        );
        let back: CheckRecord = serde_json::from_str(&text).expect("parse");
        assert_eq!(back, record);
    }

    #[test]
    fn an_item_uses_the_crate_key() {
        let item: CheckItem =
            serde_json::from_str(r#"{"entry":"alpha","crate":"alpha-core","version":"0.3.0"}"#)
                .expect("parse");
        assert_eq!(item.name, "alpha-core");
        assert!(serde_json::from_str::<CheckItem>(r#"{"entry":"a","crate":"b"}"#).is_err());
    }

    #[test]
    fn result_names_match_the_file_format() {
        for result in [
            CheckResult::Passes,
            CheckResult::Fails,
            CheckResult::Inconclusive,
        ] {
            let text = serde_json::to_string(&result).expect("serialize");
            assert_eq!(text, format!("\"{}\"", result.as_str()));
        }
    }
}
