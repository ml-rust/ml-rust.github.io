//! Usage runs: the spans in which a depender's releases name a target crate.

use jiff::{Timestamp, civil};

use super::date::add_days;
use super::lifetime::lifetime_days;
use super::uses::target_uses;
use super::weight::owner_keys;
use crate::dump::{DependencyEdge, IdMap, IdSet, Owner, VersionRow, utc_date};

/// One maximal sequence of consecutive depender releases that use a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Run {
    /// The depender crate id.
    pub(super) depender: u32,
    /// The date of the run's first release.
    pub(super) start: civil::Date,
    /// The first date the run no longer counts.
    pub(super) end: civil::Date,
    /// The run reaches the depender's latest release, so it ends by lifetime.
    pub(super) expired: bool,
}

/// The runs of every depender of each target, by target crate id.
///
/// One pass over `edges` keeps only the edges into `targets` from unyanked
/// versions of another crate. One pass over `versions` then collects the
/// unyanked releases of those depender crates.
pub(super) fn build(
    versions: &IdMap<VersionRow>,
    edges: &[DependencyEdge],
    owners: &IdMap<Vec<Owner>>,
    targets: &IdSet,
) -> IdMap<Vec<Run>> {
    let mut uses = target_uses(versions, edges, targets);
    uses.sort_unstable();
    uses.dedup();
    let dependers: IdSet = uses.iter().map(|used| used.depender).collect();
    let mut releases: IdMap<Vec<(Timestamp, u32)>> = IdMap::default();
    for (&id, version) in versions {
        if !version.yanked && dependers.contains(&version.crate_id) {
            releases
                .entry(version.crate_id)
                .or_default()
                .push((version.created_at, id));
        }
    }
    let mut out: IdMap<Vec<Run>> = IdMap::default();
    for group in uses.chunk_by(|a, b| a.depender == b.depender) {
        let Some(first) = group.first() else {
            continue;
        };
        let Some(list) = releases.get_mut(&first.depender) else {
            continue;
        };
        list.sort_unstable();
        let Some(&(_, latest)) = list.last() else {
            continue;
        };
        let latest = versions
            .get(&latest)
            .map_or("", |version| version.num.as_str());
        let multi_owner = owner_keys(owners, first.depender).len() > 1;
        let tail_days = lifetime_days(latest, list.len(), multi_owner);
        let dates: Vec<civil::Date> = list.iter().map(|&(at, _)| utc_date(at)).collect();
        let position: IdMap<usize> = list
            .iter()
            .enumerate()
            .map(|(index, &(_, id))| (id, index))
            .collect();
        for per_target in group.chunk_by(|a, b| a.target == b.target) {
            let Some(target) = per_target.first().map(|used| used.target) else {
                continue;
            };
            let mut used: Vec<usize> = per_target
                .iter()
                .filter_map(|used| position.get(&used.version).copied())
                .collect();
            used.sort_unstable();
            out.entry(target).or_default().extend(runs_of(
                first.depender,
                &dates,
                &used,
                tail_days,
            ));
        }
    }
    out
}

/// The runs of one depender over one target.
///
/// `dates` holds the dates of the depender's releases in release order.
/// `used` holds the ascending positions of the releases that use the target.
/// A run that reaches the latest release ends `tail_days` after its date.
pub(super) fn runs_of(
    depender: u32,
    dates: &[civil::Date],
    used: &[usize],
    tail_days: i64,
) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut positions = used.iter().copied().peekable();
    while let Some(first) = positions.next() {
        let mut last = first;
        while let Some(next) = positions.next_if(|&next| next <= last + 1) {
            last = next;
        }
        let Some(&start) = dates.get(first) else {
            continue;
        };
        let run = match (dates.get(last + 1), dates.get(last)) {
            (Some(&end), _) => Run {
                depender,
                start,
                end,
                expired: false,
            },
            (None, Some(&latest)) => Run {
                depender,
                start,
                end: add_days(latest, tail_days),
                expired: true,
            },
            (None, None) => continue,
        };
        runs.push(run);
    }
    runs
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::dump::{DependencyKind, OwnerKind};

    fn version(crate_id: u32, num: &str, day: i8, yanked: bool) -> VersionRow {
        let created_at = date(2026, 1, day)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .map(|zoned| zoned.timestamp())
            .unwrap_or_default();
        VersionRow {
            crate_id,
            num: num.to_owned(),
            created_at,
            yanked,
            rust_version: None,
            edition: None,
            published_by: None,
            has_lib: Some(true),
            has_bin: false,
        }
    }

    fn edge(version_id: u32, crate_id: u32, kind: DependencyKind) -> DependencyEdge {
        DependencyEdge {
            version_id,
            crate_id,
            kind,
            optional: false,
        }
    }

    fn targets(ids: &[u32]) -> IdSet {
        ids.iter().copied().collect()
    }

    #[test]
    fn dropping_and_re_adding_gives_two_runs() {
        let dates = [
            date(2026, 1, 1),
            date(2026, 2, 1),
            date(2026, 3, 1),
            date(2026, 4, 1),
        ];
        let runs = runs_of(5, &dates, &[0, 2], 60);
        assert_eq!(
            runs,
            [
                Run {
                    depender: 5,
                    start: date(2026, 1, 1),
                    end: date(2026, 2, 1),
                    expired: false,
                },
                Run {
                    depender: 5,
                    start: date(2026, 3, 1),
                    end: date(2026, 4, 1),
                    expired: false,
                },
            ]
        );
    }

    #[test]
    fn a_run_through_the_latest_release_expires_by_lifetime() {
        let dates = [date(2026, 1, 1), date(2026, 2, 1), date(2026, 3, 1)];
        let runs = runs_of(5, &dates, &[1, 2], 60);
        assert_eq!(
            runs,
            [Run {
                depender: 5,
                start: date(2026, 2, 1),
                end: date(2026, 4, 30),
                expired: true,
            }]
        );
    }

    #[test]
    fn yanked_releases_are_skipped() {
        let versions: IdMap<VersionRow> = [
            (10, version(1, "1.0.0", 1, false)),
            (11, version(1, "1.1.0", 2, true)),
            (12, version(1, "1.2.0", 3, false)),
            (13, version(1, "1.3.0", 4, false)),
        ]
        .into_iter()
        .collect();
        let edges = [
            edge(10, 9, DependencyKind::Normal),
            edge(11, 9, DependencyKind::Dev),
            edge(13, 9, DependencyKind::Build),
        ];
        let runs = build(&versions, &edges, &IdMap::default(), &targets(&[9]));
        let found = runs.get(&9).map(Vec::as_slice).unwrap_or_default();
        assert_eq!(
            found.len(),
            2,
            "the yanked 1.1.0 neither uses nor breaks a run"
        );
        assert_eq!(
            found.first().map(|run| (run.start, run.end, run.expired)),
            Some((date(2026, 1, 1), date(2026, 1, 3), false))
        );
        assert_eq!(
            found.get(1).map(|run| (run.start, run.end, run.expired)),
            Some((date(2026, 1, 4), date(2028, 1, 4), true)),
            "three unyanked stable releases count 730 days"
        );
    }

    #[test]
    fn self_dependency_and_other_targets_are_ignored() {
        let versions: IdMap<VersionRow> =
            [(10, version(9, "0.1.0", 1, false))].into_iter().collect();
        let edges = [
            edge(10, 9, DependencyKind::Normal),
            edge(10, 8, DependencyKind::Normal),
        ];
        let runs = build(&versions, &edges, &IdMap::default(), &targets(&[9]));
        assert!(runs.is_empty());
    }

    #[test]
    fn lifetime_uses_the_latest_release_and_owner_count() {
        let versions: IdMap<VersionRow> =
            [(10, version(1, "0.1.0", 1, false))].into_iter().collect();
        let owners: IdMap<Vec<Owner>> = [(
            1,
            vec![
                Owner {
                    id: 1,
                    kind: OwnerKind::User,
                },
                Owner {
                    id: 2,
                    kind: OwnerKind::User,
                },
            ],
        )]
        .into_iter()
        .collect();
        let edges = [edge(10, 9, DependencyKind::Normal)];
        let runs = build(&versions, &edges, &owners, &targets(&[9]));
        let found = runs
            .get(&9)
            .and_then(|list| list.first())
            .map(|run| run.end);
        assert_eq!(
            found,
            Some(date(2026, 3, 16)),
            "60 + 14 days after 2026-01-01"
        );
    }

    #[test]
    fn a_target_without_versions_has_no_runs() {
        assert!(runs_of(1, &[], &[0], 60).is_empty());
    }
}
