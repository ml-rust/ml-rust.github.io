//! The sandbox build check signals of a library entry.

use catalogue_core::{Entry, Signals};

use super::index::CrateIndex;
use super::link::latest_release;
use crate::dump::DumpSnapshot;
use crate::sandbox::{BuildChecks, CheckRecord, CheckResult};

/// Sets `build_fails`, `build_checked` and `build_toolchain` from the sandbox results.
///
/// Each crate of the entry counts at its latest release, the version
/// [`latest_release`] gives. A result for any other version never counts.
///
/// - A `fails` result for any crate sets `build_fails` to true, with the date and
///   toolchain of the newest such result.
/// - A `passes` result for every crate sets it to false, with the date and
///   toolchain of the oldest pass. The tier rule ages the check from that date.
/// - Otherwise it stays unknown: a crate has an `inconclusive` result, no result,
///   or no release in the dump.
pub(super) fn apply_build_checks(
    entry: &Entry,
    dump: &DumpSnapshot,
    index: &CrateIndex,
    checks: &BuildChecks,
    signals: &mut Signals,
) {
    if entry.crates.is_empty() {
        return;
    }
    let found: Vec<Option<&CheckRecord>> = entry
        .crates
        .iter()
        .map(|name| {
            let release = latest_release(dump, index, name.as_str())?;
            checks.get(&release.name, &release.version)
        })
        .collect();
    let failing = found
        .iter()
        .flatten()
        .filter(|record| record.result == CheckResult::Fails)
        .max_by_key(|record| record.checked);
    let chosen = match failing {
        Some(record) => Some((true, *record)),
        None => {
            let passes: Option<Vec<&CheckRecord>> = found
                .iter()
                .map(|record| record.filter(|r| r.result == CheckResult::Passes))
                .collect();
            passes
                .and_then(|all| all.into_iter().min_by_key(|record| record.checked))
                .map(|record| (false, record))
        }
    };
    if let Some((fails, record)) = chosen {
        signals.build_fails = Some(fails);
        signals.build_checked = Some(record.checked);
        signals.build_toolchain = Some(record.toolchain.clone());
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::Date;

    use super::*;
    use crate::assemble::fixtures::{World, day};

    fn record(name: &str, version: &str, on: i8, result: CheckResult) -> CheckRecord {
        CheckRecord {
            name: name.to_owned(),
            version: version.to_owned(),
            toolchain: format!("rustc 1.95.0 day {on}"),
            checked: Date::new(2026, 11, on).expect("date"),
            result,
            detail: String::new(),
        }
    }

    fn world() -> World {
        let mut world = World::new();
        world.dump.add_crate(1, "main");
        world.dump.add_crate(2, "helper");
        world
            .dump
            .add_version(10, 1, "1.0.0", day(2024, 1, 1), false);
        world
            .dump
            .add_version(11, 1, "1.1.0", day(2025, 1, 1), false);
        world
            .dump
            .add_version(20, 2, "0.2.0", day(2025, 1, 1), false);
        world
    }

    fn signals(world: &mut World, records: Vec<CheckRecord>) -> Signals {
        world.build_checks = BuildChecks::new(records);
        world.run(&["main", "helper"], None).signals
    }

    #[test]
    fn every_crate_passing_is_known_false_from_the_oldest_pass() {
        let mut world = world();
        let found = signals(
            &mut world,
            vec![
                record("main", "1.1.0", 2, CheckResult::Passes),
                record("helper", "0.2.0", 1, CheckResult::Passes),
            ],
        );
        assert_eq!(found.build_fails, Some(false));
        assert_eq!(found.build_checked, Date::new(2026, 11, 1).ok());
        assert_eq!(found.build_toolchain.as_deref(), Some("rustc 1.95.0 day 1"));
    }

    #[test]
    fn one_failing_crate_is_known_true() {
        let mut world = world();
        let found = signals(
            &mut world,
            vec![
                record("main", "1.1.0", 2, CheckResult::Passes),
                record("helper", "0.2.0", 1, CheckResult::Fails),
            ],
        );
        assert_eq!(found.build_fails, Some(true));
        assert_eq!(found.build_checked, Date::new(2026, 11, 1).ok());
    }

    #[test]
    fn an_older_version_or_a_missing_crate_is_unknown() {
        let mut world = world();
        let stale_version = signals(
            &mut world,
            vec![
                record("main", "1.0.0", 2, CheckResult::Fails),
                record("helper", "0.2.0", 1, CheckResult::Passes),
            ],
        );
        assert_eq!(stale_version.build_fails, None);
        assert_eq!(stale_version.build_checked, None);
        let inconclusive = signals(
            &mut world,
            vec![
                record("main", "1.1.0", 2, CheckResult::Inconclusive),
                record("helper", "0.2.0", 1, CheckResult::Passes),
            ],
        );
        assert_eq!(inconclusive.build_fails, None);
        let missing = signals(
            &mut world,
            vec![record("main", "1.1.0", 2, CheckResult::Passes)],
        );
        assert_eq!(missing.build_fails, None);
    }
}
