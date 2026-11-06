//! When each data source of `catalogue tiers` refreshes: one table, read by every fetch decision.
//!
//! No source refreshes daily, and no scheduled job runs daily.

use std::fmt;

use catalogue_signals::fallback::Source;
use clap::ValueEnum;

/// What one `catalogue tiers` run sends over the network.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum FetchMode {
    /// Fetch every source of every entry.
    #[default]
    All,
    /// The weekly scan: fetch each source that the cadence table says is due.
    Changed,
    /// Fetch only entries with no stored network values. Every other entry reuses its values.
    New,
    /// Send no request. Every entry reuses its stored values.
    None,
}

impl fmt::Display for FetchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::All => "all",
            Self::Changed => "changed",
            Self::New => "new",
            Self::None => "none",
        })
    }
}

/// When a source refreshes on the scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Cadence {
    /// Once, then only when the entry has no value, the repo shows a newer push, the dump shows a
    /// new latest version, or the value passes its [`backstop_days`].
    OnUpdate,
    /// Once, then only when the entry has no value, the dump shows a new latest version, or the
    /// value passes its [`backstop_days`]. The check reads one published release, so a push
    /// alone does not change its answer enough to pay for about three requests.
    OnRelease,
    /// On every weekly scan.
    Weekly,
}

/// A data source of one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Feed {
    /// A network source of one entry.
    Network(Source),
    /// The crates.io dump: downloads, versions and dependents.
    Dump,
    /// The RustSec archive.
    Rustsec,
}

/// The cadence of every data source. The network rows run in this order.
///
/// Every run in every mode also reads the dump and RustSec files, because the tier rules need
/// them. The scan is what keeps them current.
pub(super) const CADENCES: [(Feed, Cadence); 10] = [
    (Feed::Network(Source::RepoFacts), Cadence::Weekly),
    (Feed::Network(Source::RepoLink), Cadence::OnRelease),
    (Feed::Network(Source::Contributors), Cadence::OnUpdate),
    (Feed::Network(Source::Readme), Cadence::OnUpdate),
    (Feed::Network(Source::Releases), Cadence::Weekly),
    (Feed::Network(Source::Homebrew), Cadence::Weekly),
    (Feed::Network(Source::Repology), Cadence::Weekly),
    (Feed::Network(Source::Docker), Cadence::Weekly),
    (Feed::Dump, Cadence::Weekly),
    (Feed::Rustsec, Cadence::Weekly),
];

/// The fewest days an [`Cadence::OnUpdate`] or [`Cadence::OnRelease`] value stays in use before
/// the scan fetches it again. [`backstop_days`] adds the offset of the entry.
///
/// A push time and a version say nothing about a change outside the repo, such as an account
/// GitHub removes from a contributor list. This limit keeps any value from staying stale forever.
pub(super) const UPDATE_BACKSTOP_DAYS: i64 = 30;

/// The number of offsets [`backstop_days`] picks from: 0 to 27 days, or four weekly scans.
///
/// Values fetched in one run share a date. Without an offset, they all fall due on one scan, and
/// that scan costs about as much as a full run.
const BACKSTOP_SPREAD_DAYS: u64 = 28;

/// The FNV-1a offset basis, 64 bits.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// The FNV-1a prime, 64 bits.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The offset of `slug`, from 0 to 27 days.
///
/// FNV-1a over the slug bytes gives the same answer on every run and platform. The hasher of `std`
/// makes no such promise, so a refresh date read from it can move between runs.
fn backstop_offset(slug: &str) -> i64 {
    let hash = slug.bytes().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    });
    i64::try_from(hash % BACKSTOP_SPREAD_DAYS).unwrap_or(0)
}

/// The most days a value of `slug` stays in use before the scan fetches it again.
pub(super) fn backstop_days(slug: &str) -> i64 {
    UPDATE_BACKSTOP_DAYS.saturating_add(backstop_offset(slug))
}

/// The network sources in [`CADENCES`], in fetch order.
pub(super) fn network_sources() -> impl Iterator<Item = Source> {
    CADENCES.into_iter().filter_map(|(feed, _)| match feed {
        Feed::Network(source) => Some(source),
        Feed::Dump | Feed::Rustsec => None,
    })
}

/// The row of `source`. A source with no row reads as [`Cadence::Weekly`], so it stays current.
fn cadence_of(source: Source) -> Cadence {
    CADENCES
        .into_iter()
        .find(|(feed, _)| *feed == Feed::Network(source))
        .map_or(Cadence::Weekly, |(_, cadence)| cadence)
}

/// The most days a stored value can stand in for a failed fetch of `source`.
///
/// An on-update or on-release value of `slug` is current for its [`backstop_days`] with no fetch at all. So
/// the allowance of `max_age_days` starts there. Without this, a value 15 days old is good while
/// no fetch runs, and a failed fetch of it reads as unknown.
pub(super) fn fallback_limit(source: Source, slug: &str, max_age_days: u32) -> u32 {
    match cadence_of(source) {
        Cadence::Weekly => max_age_days,
        Cadence::OnUpdate | Cadence::OnRelease => {
            max_age_days.saturating_add(u32::try_from(backstop_days(slug)).unwrap_or(u32::MAX))
        }
    }
}

/// What the fetch decision reads about one source of one entry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Due {
    /// The entry has no stored network values.
    pub(super) new_entry: bool,
    /// The source has a stored value for this subject.
    pub(super) has_value: bool,
    /// The stored value is past the [`backstop_days`] of the entry.
    pub(super) past_backstop: bool,
    /// The repo shows a newer push.
    pub(super) pushed: bool,
    /// The dump shows a new latest version.
    pub(super) released: bool,
}

/// Whether a run in `mode` fetches `source` of an entry in the state `due`.
pub(super) fn fetches(mode: FetchMode, source: Source, due: Due) -> bool {
    match mode {
        FetchMode::All => true,
        FetchMode::None => false,
        FetchMode::New => due.new_entry,
        FetchMode::Changed => match cadence_of(source) {
            Cadence::Weekly => true,
            Cadence::OnUpdate => {
                due.new_entry || !due.has_value || due.past_backstop || due.pushed || due.released
            }
            Cadence::OnRelease => {
                due.new_entry || !due.has_value || due.past_backstop || due.released
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every network source. The match breaks the build when a new source lacks a line here.
    fn every_source() -> [Source; 8] {
        let all = [
            Source::RepoFacts,
            Source::RepoLink,
            Source::Releases,
            Source::Homebrew,
            Source::Repology,
            Source::Docker,
            Source::Contributors,
            Source::Readme,
        ];
        for source in all {
            match source {
                Source::RepoFacts
                | Source::RepoLink
                | Source::Releases
                | Source::Homebrew
                | Source::Repology
                | Source::Docker
                | Source::Contributors
                | Source::Readme => {}
            }
        }
        all
    }

    #[test]
    fn every_network_source_of_fetch_all_has_exactly_one_row() {
        let listed: Vec<Source> = network_sources().collect();
        for source in every_source() {
            let rows = listed.iter().filter(|row| **row == source).count();
            assert_eq!(rows, 1, "{source} has {rows} rows in CADENCES");
        }
        assert_eq!(listed.len(), every_source().len());
    }

    #[test]
    fn the_table_holds_the_owner_cadences() {
        for source in [Source::Readme, Source::Contributors] {
            assert_eq!(cadence_of(source), Cadence::OnUpdate, "{source}");
        }
        assert_eq!(cadence_of(Source::RepoLink), Cadence::OnRelease);
        for source in [
            Source::RepoFacts,
            Source::Releases,
            Source::Homebrew,
            Source::Repology,
            Source::Docker,
        ] {
            assert_eq!(cadence_of(source), Cadence::Weekly, "{source}");
        }
        for feed in [Feed::Dump, Feed::Rustsec] {
            assert!(CADENCES.contains(&(feed, Cadence::Weekly)), "{feed:?}");
        }
    }

    #[test]
    fn all_fetches_everything_and_none_fetches_nothing() {
        for source in every_source() {
            assert!(fetches(FetchMode::All, source, Due::default()));
            let due = Due {
                new_entry: true,
                pushed: true,
                released: true,
                past_backstop: true,
                ..Due::default()
            };
            assert!(!fetches(FetchMode::None, source, due));
        }
    }

    #[test]
    fn new_fetches_only_a_new_entry() {
        let new_entry = Due {
            new_entry: true,
            ..Due::default()
        };
        let known = Due {
            pushed: true,
            released: true,
            past_backstop: true,
            ..Due::default()
        };
        for source in every_source() {
            assert!(fetches(FetchMode::New, source, new_entry));
            assert!(!fetches(FetchMode::New, source, known));
        }
    }

    #[test]
    fn the_scan_fetches_an_on_update_source_only_when_it_is_due() {
        let quiet = Due {
            has_value: true,
            ..Due::default()
        };
        let source = Source::Contributors;
        assert!(!fetches(FetchMode::Changed, source, quiet));
        assert!(fetches(FetchMode::Changed, source, Due::default()));
        for due in [
            Due {
                pushed: true,
                ..quiet
            },
            Due {
                released: true,
                ..quiet
            },
            Due {
                past_backstop: true,
                ..quiet
            },
            Due {
                new_entry: true,
                ..quiet
            },
        ] {
            assert!(fetches(FetchMode::Changed, source, due), "{due:?}");
        }
    }

    #[test]
    fn a_push_alone_fetches_no_link_check() {
        let quiet = Due {
            has_value: true,
            ..Due::default()
        };
        let source = Source::RepoLink;
        let pushed = Due {
            pushed: true,
            ..quiet
        };
        assert!(!fetches(FetchMode::Changed, source, pushed));
        assert!(fetches(FetchMode::Changed, source, Due::default()));
        for due in [
            Due {
                released: true,
                ..quiet
            },
            Due {
                past_backstop: true,
                ..quiet
            },
        ] {
            assert!(fetches(FetchMode::Changed, source, due), "{due:?}");
        }
    }

    #[test]
    fn the_scan_fetches_a_weekly_source_with_no_change() {
        let quiet = Due {
            has_value: true,
            ..Due::default()
        };
        for source in [Source::RepoFacts, Source::Releases, Source::Docker] {
            assert!(fetches(FetchMode::Changed, source, quiet), "{source}");
        }
    }

    #[test]
    fn an_on_update_value_gets_the_backstop_on_top_of_the_fallback_limit() {
        assert_eq!(fallback_limit(Source::Homebrew, "alpha", 14), 14);
        assert_eq!(fallback_limit(Source::RepoFacts, "alpha", 14), 14);
        for source in [Source::Contributors, Source::Readme, Source::RepoLink] {
            assert_eq!(fallback_limit(source, "polars", 14), 44, "{source}");
            assert_eq!(fallback_limit(source, "alpha", 14), 63, "{source}");
        }
    }

    #[test]
    fn a_value_inside_its_backstop_can_always_fall_back() {
        for slug in ["alpha", "beta", "burn", "candle", "polars"] {
            let limit = i64::from(fallback_limit(Source::Contributors, slug, 14));
            assert!(limit > backstop_days(slug), "{slug}");
        }
    }

    #[test]
    fn the_offset_of_a_slug_is_pinned_and_in_range() {
        for (slug, offset) in [
            ("alpha", 19),
            ("beta", 19),
            ("gamma", 18),
            ("burn", 12),
            ("candle", 24),
            ("polars", 0),
        ] {
            assert_eq!(backstop_offset(slug), offset, "{slug}");
            assert_eq!(backstop_days(slug), 30 + offset, "{slug}");
        }
        for slug in ["", "a", "a-very-long-slug-with-many-parts"] {
            assert!((0..28).contains(&backstop_offset(slug)), "{slug}");
        }
    }

    #[test]
    fn the_committed_entries_spread_across_the_range() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/entries");
        let mut bands = [0_usize; 4];
        let mut total = 0_usize;
        for item in std::fs::read_dir(dir).expect("the entries directory") {
            let path = item.expect("an entry").path();
            let slug = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("a slug");
            let offset = backstop_offset(slug);
            assert!((0..28).contains(&offset), "{slug}: {offset}");
            let band = usize::try_from(offset / 7).expect("a band");
            bands[band] += 1;
            total += 1;
        }
        for count in bands {
            assert!(count * 5 >= total, "under a fifth: {bands:?} of {total}");
            assert!(count * 10 <= total * 3, "over 30%: {bands:?} of {total}");
        }
    }
}
