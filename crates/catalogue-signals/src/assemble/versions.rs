//! Release signals from the `versions` and `default_versions` tables.

use catalogue_core::{Date, Signals};

use super::index::CrateIndex;
use crate::dump::{DumpSnapshot, VersionRow, utc_date};

/// The versions of crate `crate_id`, with their ids.
fn versions<'a>(
    dump: &'a DumpSnapshot,
    index: &'a CrateIndex,
    crate_id: u32,
) -> impl Iterator<Item = (u32, &'a VersionRow)> {
    index
        .versions(crate_id)
        .iter()
        .filter_map(move |id| dump.versions.get(id).map(|row| (*id, row)))
}

/// The UTC day `row` was published.
fn published(row: &VersionRow) -> Date {
    Date::from(utc_date(row.created_at))
}

/// Sets `first_release` and `last_release` from the unyanked versions of every crate in `family`.
///
/// `first_release` is the earliest publish day and `last_release` the latest.
/// Both stay unknown when no crate in `family` has an unyanked version.
pub(super) fn family_span(
    dump: &DumpSnapshot,
    index: &CrateIndex,
    family: &[u32],
    signals: &mut Signals,
) {
    let days = move || {
        family
            .iter()
            .flat_map(move |&id| versions(dump, index, id))
            .filter(|(_, row)| !row.yanked)
            .map(|(_, row)| published(row))
    };
    signals.first_release = days().min();
    signals.last_release = days().max();
}

/// Sets the signals that come from the primary crate's versions alone.
///
/// - `latest_version`: the default version when it is unyanked, else the
///   newest unyanked version by publish time.
/// - `release_count`: the unyanked versions.
/// - `all_yanked`: the crate has versions and every one is yanked.
/// - `all_prerelease`: every unyanked version carries a pre-release suffix
///   or is `0.0.x`. Unknown when no version is unyanked.
/// - `all_zero_zero`: every unyanked version is `0.0.x`, with or without a
///   pre-release suffix. Unknown when no version is unyanked.
pub(super) fn primary_releases(
    dump: &DumpSnapshot,
    index: &CrateIndex,
    primary: u32,
    signals: &mut Signals,
) {
    let unyanked: Vec<(u32, &VersionRow)> = versions(dump, index, primary)
        .filter(|(_, row)| !row.yanked)
        .collect();
    let has_versions = versions(dump, index, primary).next().is_some();
    let default = dump
        .default_version
        .get(&primary)
        .and_then(|id| unyanked.iter().find(|(version_id, _)| version_id == id));
    let newest = unyanked
        .iter()
        .max_by_key(|(version_id, row)| (row.created_at, *version_id));
    signals.latest_version = default.or(newest).map(|(_, row)| row.num.clone());
    signals.release_count = Some(u32::try_from(unyanked.len()).unwrap_or(u32::MAX));
    signals.all_yanked = Some(has_versions && unyanked.is_empty());
    signals.all_prerelease =
        (!unyanked.is_empty()).then(|| unyanked.iter().all(|(_, row)| is_prerelease(&row.num)));
    signals.all_zero_zero =
        (!unyanked.is_empty()).then(|| unyanked.iter().all(|(_, row)| is_zero_zero(&row.num)));
}

/// Whether `num` has a pre-release suffix or is `0.0.x`. Build metadata is ignored.
fn is_prerelease(num: &str) -> bool {
    let core = num.split('+').next().unwrap_or(num);
    core.contains('-') || is_zero_zero(core)
}

/// Whether `num` is `0.0.x`, with or without a suffix.
fn is_zero_zero(num: &str) -> bool {
    num.starts_with("0.0.")
}

#[cfg(test)]
mod tests {
    use catalogue_core::EntryKind;

    use super::*;
    use crate::assemble::fixtures::{Dump, day};

    fn primary_of(dump: &Dump) -> Signals {
        let index = CrateIndex::new(&dump.snapshot);
        let mut signals = Signals::unknown(EntryKind::Library);
        primary_releases(&dump.snapshot, &index, 1, &mut signals);
        signals
    }

    #[test]
    fn family_span_reads_unyanked_versions_across_crates() {
        let mut dump = Dump::new();
        dump.add_crate(1, "a");
        dump.add_crate(2, "b");
        dump.add_version(10, 1, "0.1.0", day(2022, 1, 1), true);
        dump.add_version(11, 1, "0.2.0", day(2023, 1, 1), false);
        dump.add_version(20, 2, "0.1.0", day(2021, 6, 1), false);
        dump.add_version(21, 2, "0.2.0", day(2025, 6, 1), false);
        dump.add_version(22, 2, "0.3.0", day(2026, 1, 1), true);
        let index = CrateIndex::new(&dump.snapshot);
        let mut signals = Signals::unknown(EntryKind::Library);
        family_span(&dump.snapshot, &index, &[1, 2], &mut signals);
        assert_eq!(signals.first_release, Some(Date::from(day(2021, 6, 1))));
        assert_eq!(signals.last_release, Some(Date::from(day(2025, 6, 1))));
        family_span(&dump.snapshot, &index, &[9], &mut signals);
        assert_eq!((signals.first_release, signals.last_release), (None, None));
    }

    #[test]
    fn latest_version_prefers_the_unyanked_default() {
        let mut dump = Dump::new();
        dump.add_crate(1, "a");
        dump.add_version(10, 1, "1.0.0", day(2024, 1, 1), false);
        dump.add_version(11, 1, "0.9.9", day(2025, 1, 1), false);
        dump.add_version(12, 1, "2.0.0-rc.1", day(2025, 6, 1), false);
        dump.add_version(13, 1, "1.1.0", day(2025, 7, 1), true);
        dump.set_default(1, 10);
        let signals = primary_of(&dump);
        assert_eq!(signals.latest_version.as_deref(), Some("1.0.0"));
        assert_eq!(signals.release_count, Some(3));
        assert_eq!(signals.all_yanked, Some(false));
        assert_eq!(signals.all_prerelease, Some(false));
        dump.set_default(1, 13);
        let signals = primary_of(&dump);
        assert_eq!(signals.latest_version.as_deref(), Some("2.0.0-rc.1"));
    }

    #[test]
    fn every_version_yanked() {
        let mut dump = Dump::new();
        dump.add_crate(1, "a");
        dump.add_version(10, 1, "1.0.0", day(2024, 1, 1), true);
        let signals = primary_of(&dump);
        assert_eq!(signals.all_yanked, Some(true));
        assert_eq!(signals.release_count, Some(0));
        assert_eq!(signals.latest_version, None);
        assert_eq!(signals.all_prerelease, None);
        assert_eq!(signals.all_zero_zero, None);
    }

    #[test]
    fn a_crate_without_versions_is_not_all_yanked() {
        let mut dump = Dump::new();
        dump.add_crate(1, "a");
        assert_eq!(primary_of(&dump).all_yanked, Some(false));
    }

    #[test]
    fn prerelease_and_zero_zero_versions_are_all_prerelease() {
        let mut dump = Dump::new();
        dump.add_crate(1, "a");
        dump.add_version(10, 1, "0.0.3", day(2024, 1, 1), false);
        dump.add_version(11, 1, "0.1.0-alpha.1+build.5", day(2024, 2, 1), false);
        dump.add_version(12, 1, "0.1.0", day(2024, 3, 1), true);
        let found = primary_of(&dump);
        assert_eq!(found.all_prerelease, Some(true));
        assert_eq!(found.all_zero_zero, Some(false));
        assert!(!is_prerelease("1.0.0+build-7"));
        assert!(is_prerelease("1.0.0-rc.1"));
        assert!(!is_prerelease("0.1.0"));
    }

    #[test]
    fn only_zero_zero_versions_are_all_zero_zero() {
        let mut dump = Dump::new();
        dump.add_crate(1, "a");
        dump.add_version(10, 1, "0.0.1", day(2024, 1, 1), false);
        dump.add_version(11, 1, "0.0.2-rc.1", day(2024, 2, 1), false);
        dump.add_version(12, 1, "1.0.0", day(2024, 3, 1), true);
        let found = primary_of(&dump);
        assert_eq!(found.all_prerelease, Some(true));
        assert_eq!(found.all_zero_zero, Some(true));
    }

    #[test]
    fn release_candidates_alone_are_prerelease_and_not_zero_zero() {
        let mut dump = Dump::new();
        dump.add_crate(1, "a");
        dump.add_version(10, 1, "2.0.0-rc.1", day(2024, 1, 1), false);
        let found = primary_of(&dump);
        assert_eq!(found.all_prerelease, Some(true));
        assert_eq!(found.all_zero_zero, Some(false));
    }

    #[test]
    fn a_stable_version_clears_both_facts() {
        let mut dump = Dump::new();
        dump.add_crate(1, "a");
        dump.add_version(10, 1, "0.0.1", day(2024, 1, 1), false);
        dump.add_version(11, 1, "0.1.0", day(2024, 2, 1), false);
        let found = primary_of(&dump);
        assert_eq!(found.all_prerelease, Some(false));
        assert_eq!(found.all_zero_zero, Some(false));
    }
}
