//! Computes the adoption signals of a set of target crates from one dump.

use jiff::civil;

use super::runs::{self, Run};
use super::series::{Usage, usage};
use super::weight::{self, OwnerKey};
use crate::dump::{DumpSnapshot, IdMap, IdSet, VersionRow};

/// The adoption signals of each crate in `targets`, evaluated on `today`.
///
/// Callers pass the dump's UTC date as `today`. Targets absent from
/// `snapshot.crates` are skipped. The edges and versions are each read once
/// for the whole set.
pub fn compute(snapshot: &DumpSnapshot, targets: &IdSet, today: civil::Date) -> IdMap<Usage> {
    let targets: IdSet = targets
        .iter()
        .copied()
        .filter(|id| snapshot.crates.contains_key(id))
        .collect();
    let runs = runs::build(
        &snapshot.versions,
        &snapshot.edges,
        &snapshot.owners,
        &targets,
    );
    let dependers: IdSet = runs.values().flatten().map(|run| run.depender).collect();
    let depender_owners: IdMap<Vec<OwnerKey>> = dependers
        .into_iter()
        .map(|depender| (depender, weight::owner_keys(&snapshot.owners, depender)))
        .collect();
    let publishers = publishers(&snapshot.versions, &targets);
    let shared = weight::rust_bus(&snapshot.users, &snapshot.teams);
    targets
        .into_iter()
        .map(|target| {
            let excluded = weight::excluded(
                &weight::owner_keys(&snapshot.owners, target),
                publishers
                    .get(&target)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
                &shared,
            );
            let weigh = |ids: &[u32]| weight::users(ids, &depender_owners, &excluded);
            let target_runs: &[Run] = runs.get(&target).map(Vec::as_slice).unwrap_or_default();
            (target, usage(target_runs, today, &weigh))
        })
        .collect()
}

/// The distinct publishing user ids of each target's versions, yanked or not.
fn publishers(versions: &IdMap<VersionRow>, targets: &IdSet) -> IdMap<Vec<u32>> {
    let mut out: IdMap<Vec<u32>> = IdMap::default();
    for version in versions.values() {
        if let Some(user) = version.published_by
            && targets.contains(&version.crate_id)
        {
            out.entry(version.crate_id).or_default().push(user);
        }
    }
    for users in out.values_mut() {
        users.sort_unstable();
        users.dedup();
    }
    out
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;

    use super::*;

    fn version(crate_id: u32, published_by: Option<u32>) -> VersionRow {
        VersionRow {
            crate_id,
            num: "1.0.0".to_owned(),
            created_at: Timestamp::UNIX_EPOCH,
            yanked: false,
            rust_version: None,
            edition: None,
            published_by,
            has_lib: Some(true),
            has_bin: false,
        }
    }

    #[test]
    fn publishers_are_kept_per_target_and_distinct() {
        let versions: IdMap<VersionRow> = [
            (1, version(9, Some(4))),
            (2, version(9, Some(4))),
            (3, version(9, None)),
            (4, version(8, Some(5))),
        ]
        .into_iter()
        .collect();
        let targets: IdSet = [9].into_iter().collect();
        let found = publishers(&versions, &targets);
        assert_eq!(found.get(&9).map(Vec::as_slice), Some(&[4][..]));
        assert_eq!(found.len(), 1);
    }
}
