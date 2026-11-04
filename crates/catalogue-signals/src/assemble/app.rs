//! The signals of an app entry.

use catalogue_core::{Date, Entry, EntryKind, Signals};

use super::assembled::Assembled;
use super::facts::apply_facts;
use super::library::{listed_crates, month_downloads};
use super::link::apply_link;
use super::main_crate::main_crate_left_out;
use super::sources::LibrarySources;
use super::versions::{family_span, primary_releases};
use crate::history::{ReleaseValues, RepoValues};
use crate::link::LinkCheck;

/// The signals of an app from its fetched sources.
///
/// - Repository signals come from `facts`, as for a library.
/// - `binary_downloads_last3`: the binary downloads over the sampled releases.
/// - `binary_downloads_per_release`: that total divided by the number of
///   sampled releases. Unknown when there is no release.
/// - `last_release`: the newest publish day among the sampled releases.
/// - `homebrew_on_request_365d`: installs on request over 365 days.
/// - `repology_families`: the repository families that package the app.
/// - `docker_pull_growth_90d`: `docker` minus the pull count of `baseline`, the
///   [`History::docker_baseline`](crate::history::History::docker_baseline)
///   snapshot. A count below the baseline reads as `Some(0)` with a note.
///   Unknown without both counts.
///
/// A `None` source leaves its signals unknown. The caller notes why a source is
/// missing, so the notes stay empty. Only a falling pull count adds a note.
pub fn app_signals(
    facts: Option<&RepoValues>,
    releases: Option<&ReleaseValues>,
    homebrew: Option<u64>,
    repology: Option<u32>,
    docker: Option<u64>,
    baseline: Option<(Date, u64)>,
) -> Assembled {
    let mut signals = Signals::unknown(EntryKind::App);
    apply_facts(facts, &mut signals);
    if let Some(found) = releases {
        let total = found.binary_downloads;
        signals.binary_downloads_last3 = Some(total);
        signals.binary_downloads_per_release = total.checked_div(u64::from(found.releases));
        signals.last_release = found.last_release;
    }
    signals.homebrew_on_request_365d = homebrew;
    signals.repology_families = repology;
    let mut notes = Vec::new();
    if let (Some(current), Some((date, before))) = (docker, baseline) {
        signals.docker_pull_growth_90d = Some(current.saturating_sub(before));
        if current < before {
            notes.push(format!(
                "Docker pulls fell from {before} on {date} to {current}: growth reads as 0"
            ));
        }
    }
    Assembled { signals, notes }
}

/// Adds what the crates of app `entry` state to the `assembled` signals of [`app_signals`].
///
/// An app with no crate stays exactly as it is. An app with crates reads the dump the way a
/// library does:
///
/// - `first_release`, `last_release`: the span of the family, joined with the GitHub dates of
///   `releases`. The earlier first date and the later last date win.
/// - `latest_version`, `release_count`, `all_prerelease`, `all_zero_zero`: the primary's versions.
/// - `downloads_per_month`: the primary's last 30 days. No tier rule reads it for an app.
/// - `repo_link_verified`: `link.verified()`. No tier rule reads it for an app.
/// - `main_crate_left_out`: a crate with the repository's own name that the entry does not list.
///
/// `all_yanked` and `crate_deleted` stay unknown. They end a library, and a yanked crate says
/// nothing about the program. Each crate missing from the dump adds a note.
pub fn app_crate_signals(
    entry: &Entry,
    sources: &LibrarySources<'_>,
    releases: Option<&ReleaseValues>,
    link: Option<&LinkCheck>,
    assembled: &mut Assembled,
) {
    let Some(primary) = entry.crates.first() else {
        return;
    };
    let Assembled { signals, notes } = assembled;
    let (dump, index) = (sources.dump, sources.index);
    apply_link(link, signals, notes);
    signals.main_crate_left_out = main_crate_left_out(entry, dump, index);
    let family = listed_crates(entry, primary, index, notes);
    let github_last = signals.last_release;
    family_span(dump, index, &family, signals);
    signals.last_release = signals.last_release.max(github_last);
    signals.first_release = earlier(
        signals.first_release,
        releases.and_then(|found| found.first_release),
    );
    if let Some(id) = index.id(primary.as_str()) {
        primary_releases(dump, index, id, signals);
        signals.all_yanked = None;
        signals.downloads_per_month = Some(month_downloads(dump, id));
    }
}

/// The earlier of two dates. An unknown date never wins.
fn earlier(a: Option<Date>, b: Option<Date>) -> Option<Date> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::Date;

    use super::*;
    use crate::assemble::fixtures::{World, day, found_facts};
    use crate::distribution::{ReleaseDownloads, ReleaseSummary};
    use crate::github::RepoFacts;

    fn release(published: Option<Date>, downloads: u64) -> ReleaseSummary {
        ReleaseSummary {
            tag: "v1".to_owned(),
            published,
            prerelease: false,
            binary_downloads: downloads,
        }
    }

    #[test]
    fn every_source_feeds_its_signal() {
        let releases = ReleaseDownloads {
            releases: vec![
                release(Some(Date::from(day(2026, 10, 1))), 500),
                release(Some(Date::from(day(2026, 6, 1))), 300),
                release(None, 100),
            ],
            total_binary_downloads: 900,
        };
        let facts = RepoValues::from(&RepoFacts::Found(found_facts()));
        let assembled = app_signals(
            Some(&facts),
            Some(&ReleaseValues::from(&releases)),
            Some(640),
            Some(2),
            None,
            None,
        );
        let found = assembled.signals;
        assert_eq!(found.kind, EntryKind::App);
        assert_eq!(found.binary_downloads_last3, Some(900));
        assert_eq!(found.binary_downloads_per_release, Some(300));
        assert_eq!(found.last_release, Some(Date::from(day(2026, 10, 1))));
        assert_eq!(found.homebrew_on_request_365d, Some(640));
        assert_eq!(found.repology_families, Some(2));
        assert_eq!(found.docker_pull_growth_90d, None);
        assert_eq!(found.repo_gone, Some(false));
        assert!(assembled.notes.is_empty(), "{:?}", assembled.notes);
    }

    #[test]
    fn no_release_has_a_total_but_no_per_release_count() {
        let releases = ReleaseDownloads {
            releases: Vec::new(),
            total_binary_downloads: 0,
        };
        let values = ReleaseValues::from(&releases);
        let found = app_signals(None, Some(&values), None, None, None, None).signals;
        assert_eq!(found.binary_downloads_last3, Some(0));
        assert_eq!(found.binary_downloads_per_release, None);
        assert_eq!(found.last_release, None);
    }

    #[test]
    fn missing_sources_stay_unknown() {
        let assembled = app_signals(None, None, None, None, None, None);
        assert_eq!(assembled.signals, Signals::unknown(EntryKind::App));
        assert!(assembled.notes.is_empty());
    }

    fn growth(docker: Option<u64>, baseline: Option<(Date, u64)>) -> Assembled {
        app_signals(None, None, None, None, docker, baseline)
    }

    #[test]
    fn growth_is_the_current_count_minus_the_baseline() {
        let before = Date::from(day(2026, 8, 1));
        let assembled = growth(Some(1_500), Some((before, 1_000)));
        assert_eq!(assembled.signals.docker_pull_growth_90d, Some(500));
        assert!(assembled.notes.is_empty());
        let flat = growth(Some(1_000), Some((before, 1_000)));
        assert_eq!(flat.signals.docker_pull_growth_90d, Some(0));
        assert!(flat.notes.is_empty());
    }

    #[test]
    fn a_falling_count_reads_as_zero_growth_with_a_note() {
        let before = Date::from(day(2026, 8, 1));
        let assembled = growth(Some(900), Some((before, 1_000)));
        assert_eq!(assembled.signals.docker_pull_growth_90d, Some(0));
        assert_eq!(
            assembled.notes,
            ["Docker pulls fell from 1000 on 2026-08-01 to 900: growth reads as 0"]
        );
    }

    fn on(year: i16, month: i8, date: i8) -> Option<Date> {
        Some(Date::from(day(year, month, date)))
    }

    fn release_values(first: Option<Date>, last: Option<Date>) -> ReleaseValues {
        ReleaseValues {
            binary_downloads: 0,
            releases: 2,
            first_release: first,
            last_release: last,
        }
    }

    /// A dump with a crate `tool` released in 2024 and 2025, and a helper released in 2023.
    fn world() -> World {
        let mut world = World::new();
        world.dump.add_crate(1, "tool");
        world.dump.add_crate(2, "tool-core");
        world
            .dump
            .add_version(10, 1, "1.0.0", day(2024, 5, 1), false);
        world
            .dump
            .add_version(11, 1, "1.1.0", day(2025, 5, 1), false);
        world
            .dump
            .add_version(20, 2, "0.1.0", day(2023, 5, 1), false);
        world.dump.set_default(1, 11);
        world
            .dump
            .set_downloads(1, day(2026, 10, 3), vec![2_u32; 30]);
        world
    }

    #[test]
    fn an_app_with_crates_reads_its_release_facts_from_the_dump() {
        let assembled = world().run_app(&["tool", "tool-core"], None, None);
        let found = assembled.signals;
        assert_eq!(found.first_release, on(2023, 5, 1));
        assert_eq!(found.last_release, on(2025, 5, 1));
        assert_eq!(found.latest_version.as_deref(), Some("1.1.0"));
        assert_eq!(found.release_count, Some(2));
        assert_eq!(found.all_prerelease, Some(false));
        assert_eq!(found.all_zero_zero, Some(false));
        assert_eq!(found.downloads_per_month, Some(60));
        assert_eq!(found.kind, EntryKind::App);
        assert!(assembled.notes.is_empty(), "{:?}", assembled.notes);
    }

    #[test]
    fn the_later_last_release_and_the_earlier_first_release_win() {
        let world = world();
        let github_newer = release_values(on(2020, 1, 1), on(2026, 9, 1));
        let found = world.run_app(&["tool"], Some(&github_newer), None).signals;
        assert_eq!(found.last_release, on(2026, 9, 1));
        assert_eq!(found.first_release, on(2020, 1, 1));
        let dump_newer = release_values(on(2025, 1, 1), on(2025, 1, 2));
        let found = world.run_app(&["tool"], Some(&dump_newer), None).signals;
        assert_eq!(found.last_release, on(2025, 5, 1));
        assert_eq!(found.first_release, on(2024, 5, 1));
        let no_dates = release_values(None, None);
        let found = world.run_app(&["tool"], Some(&no_dates), None).signals;
        assert_eq!(found.last_release, on(2025, 5, 1));
        assert_eq!(found.first_release, on(2024, 5, 1));
    }

    #[test]
    fn an_app_without_crates_stays_as_it_was() {
        let values = release_values(on(2020, 1, 1), on(2026, 9, 1));
        let assembled = world().run_app(&[], Some(&values), None);
        let alone = app_signals(None, Some(&values), None, None, None, None);
        assert_eq!(assembled, alone);
        assert_eq!(assembled.signals.first_release, None);
        assert_eq!(assembled.signals.last_release, on(2026, 9, 1));
        assert_eq!(assembled.signals.downloads_per_month, None);
        assert_eq!(assembled.signals.latest_version, None);
        assert_eq!(assembled.signals.all_prerelease, None);
    }

    #[test]
    fn prerelease_and_zero_zero_versions_reach_the_release_gate() {
        let mut world = World::new();
        world.dump.add_crate(1, "tool");
        world
            .dump
            .add_version(10, 1, "0.1.0-rc.1", day(2025, 1, 1), false);
        let found = world.run_app(&["tool"], None, None).signals;
        assert_eq!(found.all_prerelease, Some(true));
        assert_eq!(found.all_zero_zero, Some(false));
        let mut world = World::new();
        world.dump.add_crate(1, "tool");
        world
            .dump
            .add_version(10, 1, "0.0.1", day(2025, 1, 1), false);
        let found = world.run_app(&["tool"], None, None).signals;
        assert_eq!(found.all_prerelease, Some(true));
        assert_eq!(found.all_zero_zero, Some(true));
    }

    #[test]
    fn a_yanked_or_deleted_crate_never_ends_the_program() {
        let mut world = World::new();
        world.dump.add_crate(1, "tool");
        world
            .dump
            .add_version(10, 1, "1.0.0", day(2025, 1, 1), true);
        let found = world.run_app(&["tool"], None, None).signals;
        assert_eq!(found.all_yanked, None);
        assert_eq!(found.crate_deleted, None);
        assert_eq!(found.release_count, Some(0));
        let mut world = World::new();
        world.dump.add_deleted("tool");
        let gone = world.run_app(&["tool"], None, None);
        assert_eq!(gone.signals.crate_deleted, None);
        assert_eq!(gone.notes, ["crate tool is deleted from crates.io"]);
    }

    #[test]
    fn a_crate_missing_from_the_dump_is_noted_and_leaves_its_facts_unknown() {
        let assembled = World::new().run_app(&["tool"], None, None);
        assert_eq!(assembled.notes, ["crate tool is not in the dump"]);
        assert_eq!(assembled.signals.all_prerelease, None);
        assert_eq!(assembled.signals.downloads_per_month, None);
    }

    #[test]
    fn the_link_and_the_main_crate_read_like_a_library() {
        let mut world = world();
        world.dump.add_crate(1, "tool").repository =
            Some("https://github.com/ml-rust/tool".to_owned());
        world.dump.add_crate(4, "helper");
        let link = LinkCheck::NoVcsInfo;
        let assembled = world.run_app(&["helper"], None, Some(&link));
        assert_eq!(
            assembled.signals.main_crate_left_out.as_deref(),
            Some("tool")
        );
        assert_eq!(assembled.signals.repo_link_verified, link.verified());
        assert_eq!(assembled.notes.len(), 1, "{:?}", assembled.notes);
        let listed = world.run_app(&["tool"], None, None);
        assert_eq!(listed.signals.main_crate_left_out, None);
        assert_eq!(listed.signals.repo_link_verified, None);
    }

    #[test]
    fn growth_needs_both_counts() {
        let before = Date::from(day(2026, 8, 1));
        for assembled in [
            growth(None, Some((before, 1_000))),
            growth(Some(1_000), None),
            growth(None, None),
        ] {
            assert_eq!(assembled.signals.docker_pull_growth_90d, None);
            assert!(assembled.notes.is_empty());
        }
    }
}
