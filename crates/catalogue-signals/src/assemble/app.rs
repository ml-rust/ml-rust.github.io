//! The signals of an app entry.

use catalogue_core::{EntryKind, Signals};

use super::assembled::Assembled;
use super::facts::apply_facts;
use crate::distribution::{HomebrewInstalls, ReleaseDownloads, RepologyPackaging};
use crate::github::RepoFacts;

/// The signals of an app from its fetched sources.
///
/// - Repository signals come from `facts`, as for a library.
/// - `binary_downloads_last3`: the binary downloads over the sampled releases.
/// - `binary_downloads_per_release`: that total divided by the number of
///   sampled releases. Unknown when there is no release.
/// - `last_release`: the newest publish day among the sampled releases.
/// - `homebrew_on_request_365d`: installs on request over 365 days.
/// - `repology_families`: the repository families that package the app.
/// - `docker_pull_growth_90d`: unknown. Growth needs two pull counts 90 days apart.
///
/// A `None` source leaves its signals unknown. The caller notes why a source is
/// missing, so the notes stay empty.
pub fn app_signals(
    facts: Option<&RepoFacts>,
    releases: Option<&ReleaseDownloads>,
    homebrew: Option<&HomebrewInstalls>,
    repology: Option<&RepologyPackaging>,
) -> Assembled {
    let mut signals = Signals::unknown(EntryKind::App);
    apply_facts(facts, &mut signals);
    if let Some(found) = releases {
        let total = found.total_binary_downloads;
        signals.binary_downloads_last3 = Some(total);
        let count = u64::try_from(found.releases.len()).unwrap_or(u64::MAX);
        signals.binary_downloads_per_release = total.checked_div(count);
        signals.last_release = found.releases.iter().filter_map(|r| r.published).max();
    }
    signals.homebrew_on_request_365d = homebrew.map(|found| found.on_request_365d);
    signals.repology_families = repology.map(|found| found.families);
    Assembled {
        signals,
        notes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::Date;

    use super::*;
    use crate::assemble::fixtures::{day, found_facts};
    use crate::distribution::ReleaseSummary;

    fn release(published: Option<Date>, downloads: u64) -> ReleaseSummary {
        ReleaseSummary {
            tag: "v1".to_owned(),
            published,
            prerelease: false,
            binary_downloads: downloads,
        }
    }

    fn homebrew(on_request_365d: u64) -> HomebrewInstalls {
        HomebrewInstalls {
            install_30d: 0,
            install_90d: 0,
            install_365d: 0,
            on_request_30d: 0,
            on_request_90d: 0,
            on_request_365d,
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
        let packaging = RepologyPackaging {
            repos: vec!["arch".to_owned(), "debian_12".to_owned()],
            families: 2,
        };
        let facts = RepoFacts::Found(found_facts());
        let assembled = app_signals(
            Some(&facts),
            Some(&releases),
            Some(&homebrew(640)),
            Some(&packaging),
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
        let found = app_signals(None, Some(&releases), None, None).signals;
        assert_eq!(found.binary_downloads_last3, Some(0));
        assert_eq!(found.binary_downloads_per_release, None);
        assert_eq!(found.last_release, None);
    }

    #[test]
    fn missing_sources_stay_unknown() {
        let assembled = app_signals(None, None, None, None);
        assert_eq!(assembled.signals, Signals::unknown(EntryKind::App));
        assert!(assembled.notes.is_empty());
    }
}
