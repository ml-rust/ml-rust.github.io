//! The sandbox results a tier run reads, keyed by crate release.

use std::collections::HashMap;

use catalogue_core::canonical_crate_name;

use super::record::CheckRecord;

/// Sandbox results by crate release. Empty when the run reads no result file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildChecks {
    by_release: HashMap<(String, String), CheckRecord>,
}

impl BuildChecks {
    /// The results in `records`. For a release checked twice, the later date wins,
    /// and on the same date the later record wins.
    pub fn new(records: Vec<CheckRecord>) -> Self {
        let mut by_release: HashMap<(String, String), CheckRecord> = HashMap::new();
        for record in records {
            let key = (canonical_crate_name(&record.name), record.version.clone());
            let newer = by_release
                .get(&key)
                .is_some_and(|found| found.checked > record.checked);
            if !newer {
                by_release.insert(key, record);
            }
        }
        Self { by_release }
    }

    /// The result for crate `name` at `version`, in any spelling crates.io treats as equal.
    ///
    /// A result for another version of the crate never matches.
    pub fn get(&self, name: &str, version: &str) -> Option<&CheckRecord> {
        self.by_release
            .get(&(canonical_crate_name(name), version.to_owned()))
    }

    /// Whether no result is held.
    pub fn is_empty(&self) -> bool {
        self.by_release.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::Date;

    use super::*;
    use crate::sandbox::CheckResult;

    fn record(name: &str, version: &str, day: i8, result: CheckResult) -> CheckRecord {
        CheckRecord {
            name: name.to_owned(),
            version: version.to_owned(),
            toolchain: "rustc 1.95.0".to_owned(),
            checked: Date::new(2026, 11, day).expect("date"),
            result,
            detail: String::new(),
        }
    }

    #[test]
    fn a_release_matches_by_name_and_exact_version() {
        let checks = BuildChecks::new(vec![record("foo_bar", "1.0.0", 1, CheckResult::Fails)]);
        assert!(checks.get("Foo-Bar", "1.0.0").is_some());
        assert!(checks.get("foo-bar", "1.0.1").is_none());
        assert!(BuildChecks::default().is_empty());
    }

    #[test]
    fn the_later_check_of_a_release_wins() {
        let checks = BuildChecks::new(vec![
            record("a", "1.0.0", 2, CheckResult::Fails),
            record("a", "1.0.0", 1, CheckResult::Passes),
            record("a", "1.0.0", 2, CheckResult::Inconclusive),
        ]);
        let found = checks.get("a", "1.0.0").map(|r| r.result);
        assert_eq!(found, Some(CheckResult::Inconclusive));
    }
}
