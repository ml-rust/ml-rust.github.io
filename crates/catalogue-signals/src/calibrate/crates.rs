//! The crate-signal samples of a population.

use super::distribution::Distribution;
use crate::discover::Candidate;
use crate::dump::{DumpSnapshot, IdMap};
use crate::usage::Usage;

/// The signal samples of the crates in the population.
///
/// The population is the crates.io discovery candidates with at least one
/// active user, which means `current >= 1`. Each vector holds one value per
/// crate, in candidate order. A NaN value is left out of its vector, so the
/// vectors can differ in length.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrateSamples {
    /// The weighted external users on the dump date.
    pub active_users: Vec<f64>,
    /// The weighted users from runs that started in the last 365 days.
    pub new_adopters: Vec<f64>,
    /// The current users against the peak, from 0 to 1.
    pub former_glory: Vec<f64>,
    /// The versions of the crate that are not yanked.
    pub release_count: Vec<f64>,
}

impl CrateSamples {
    /// The distribution of each signal at `points`, in field order.
    pub fn distributions(&self, points: &[f64]) -> Vec<Distribution> {
        [
            ("active_users", &self.active_users),
            ("new_adopters", &self.new_adopters),
            ("former_glory", &self.former_glory),
            ("release_count", &self.release_count),
        ]
        .into_iter()
        .map(|(name, samples)| Distribution::from_samples(name, samples, points))
        .collect()
    }
}

/// Collects the signals of the candidates that have at least one active user.
///
/// A candidate with no entry in `usage`, or with `current` below 1 or NaN, is
/// not in the population. `release_count` counts the versions in
/// `snapshot.versions` that are not yanked.
pub fn crate_samples(
    snapshot: &DumpSnapshot,
    candidates: &[Candidate],
    usage: &IdMap<Usage>,
) -> CrateSamples {
    let mut releases: IdMap<u32> = IdMap::default();
    for version in snapshot.versions.values().filter(|v| !v.yanked) {
        *releases.entry(version.crate_id).or_insert(0) += 1;
    }
    let mut samples = CrateSamples::default();
    for candidate in candidates {
        let Some(found) = usage.get(&candidate.crate_id) else {
            continue;
        };
        if found.current.is_nan() || found.current < 1.0 {
            continue;
        }
        let count = releases.get(&candidate.crate_id).copied().unwrap_or(0);
        push(&mut samples.active_users, found.current);
        push(&mut samples.new_adopters, found.new_adopters);
        push(&mut samples.former_glory, found.former_glory);
        push(&mut samples.release_count, f64::from(count));
    }
    samples
}

/// Adds `value` unless it is NaN.
fn push(samples: &mut Vec<f64>, value: f64) {
    if !value.is_nan() {
        samples.push(value);
    }
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;

    use super::*;
    use crate::dump::{CrateRow, DumpMeta, VersionRow};

    fn candidate(crate_id: u32) -> Candidate {
        Candidate {
            crate_id,
            name: format!("c{crate_id}"),
            reasons: Vec::new(),
            deprecated: false,
        }
    }

    fn usage(current: f64, new_adopters: f64, former_glory: f64) -> Usage {
        Usage {
            current,
            peak: current,
            former_glory,
            growth: 1.0,
            new_adopters,
            removed: 0,
            expired: 0,
            history: Vec::new(),
        }
    }

    fn version(crate_id: u32, yanked: bool) -> VersionRow {
        VersionRow {
            crate_id,
            num: "1.0.0".to_owned(),
            created_at: Timestamp::UNIX_EPOCH,
            yanked,
            rust_version: None,
            edition: None,
            published_by: None,
        }
    }

    fn snapshot(versions: &[(u32, VersionRow)]) -> DumpSnapshot {
        DumpSnapshot {
            meta: DumpMeta {
                timestamp: Timestamp::UNIX_EPOCH,
                crates_io_commit: String::new(),
                directory: String::new(),
            },
            crates: IdMap::<CrateRow>::default(),
            versions: versions.iter().cloned().collect(),
            default_version: IdMap::default(),
            edges: Vec::new(),
            default_requirements: IdMap::default(),
            owners: IdMap::default(),
            users: IdMap::default(),
            teams: IdMap::default(),
            categories: IdMap::default(),
            keywords: IdMap::default(),
            deleted: Vec::new(),
            downloads: IdMap::default(),
            warnings: Default::default(),
        }
    }

    #[test]
    fn only_candidates_with_an_active_user_are_sampled() {
        let snapshot = snapshot(&[
            (1, version(1, false)),
            (2, version(1, false)),
            (3, version(1, true)),
            (4, version(2, false)),
            (5, version(3, false)),
        ]);
        let candidates = [candidate(1), candidate(2), candidate(3), candidate(4)];
        let usage: IdMap<Usage> = [
            (1, usage(3.0, 1.0, 0.5)),
            (2, usage(0.99, 0.0, 1.0)),
            (3, usage(1.0, 0.0, 1.0)),
            (9, usage(5.0, 5.0, 1.0)),
        ]
        .into_iter()
        .collect();
        let samples = crate_samples(&snapshot, &candidates, &usage);
        assert_eq!(samples.active_users, [3.0, 1.0]);
        assert_eq!(samples.new_adopters, [1.0, 0.0]);
        assert_eq!(samples.former_glory, [0.5, 1.0]);
        assert_eq!(samples.release_count, [2.0, 1.0]);
    }

    #[test]
    fn nan_values_are_left_out() {
        let snapshot = snapshot(&[(1, version(1, false))]);
        let usage: IdMap<Usage> = [
            (1, usage(2.0, f64::NAN, 1.0)),
            (2, usage(f64::NAN, 1.0, 1.0)),
        ]
        .into_iter()
        .collect();
        let samples = crate_samples(&snapshot, &[candidate(1), candidate(2)], &usage);
        assert_eq!(samples.active_users, [2.0]);
        assert!(samples.new_adopters.is_empty());
        assert_eq!(samples.former_glory, [1.0]);
        assert_eq!(samples.release_count, [1.0]);
    }

    #[test]
    fn distributions_follow_the_field_order() {
        let samples = CrateSamples {
            active_users: vec![1.0, 2.0],
            ..CrateSamples::default()
        };
        let found = samples.distributions(&[50.0]);
        let names: Vec<&str> = found.iter().map(|d| d.name).collect();
        assert_eq!(
            names,
            [
                "active_users",
                "new_adopters",
                "former_glory",
                "release_count"
            ]
        );
        assert_eq!(found.first().map(|d| d.count), Some(2));
        assert_eq!(found.get(1).map(|d| d.count), Some(0));
    }
}
