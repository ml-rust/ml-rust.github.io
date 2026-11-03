//! Helpers shared by the integration tests.

use std::path::PathBuf;

use catalogue::{Outcome, validate};

/// Path of the fixture data directory named `case`.
pub fn fixture(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(case)
}

/// The `ok: ...` line for a fixture that passes every rule.
pub fn valid_summary(case: &str) -> String {
    match validate(&fixture(case)) {
        Ok(Outcome::Valid(summary)) => summary.to_string(),
        Ok(Outcome::Invalid(diagnostics)) => panic!("`{case}` is invalid:\n{diagnostics}"),
        Err(error) => panic!("`{case}` cannot be read: {error}"),
    }
}

/// Every diagnostic for a fixture that breaks at least one rule.
pub fn diagnostics(case: &str) -> Vec<String> {
    match validate(&fixture(case)) {
        Ok(Outcome::Invalid(diagnostics)) => diagnostics.iter().map(ToString::to_string).collect(),
        Ok(Outcome::Valid(summary)) => panic!("`{case}` is valid: {summary}"),
        Err(error) => panic!("`{case}` cannot be read: {error}"),
    }
}

/// Asserts that a fixture yields exactly `count` diagnostics and that each contains `needle`.
pub fn expect_invalid(case: &str, needle: &str, count: usize) {
    let found = diagnostics(case);
    assert_eq!(found.len(), count, "`{case}`: {found:?}");
    for message in &found {
        assert!(
            message.contains(needle),
            "`{case}`: `{message}` lacks `{needle}`"
        );
    }
}
