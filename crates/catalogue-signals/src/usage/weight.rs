//! Owner weighting: active depender crates become a count of external users.

use std::collections::{BTreeMap, HashSet};

use rustc_hash::FxBuildHasher;

use crate::dump::{IdMap, Owner, OwnerKind};

/// The most one owner adds to a crate's user count.
pub(super) const OWNER_CAP: f64 = 1.0;
/// The login of the shared crates.io maintenance account, never counted as a user.
pub(super) const RUST_BUS_USER: &str = "rust-bus";
/// The login prefix of the shared maintenance teams, never counted as users.
pub(super) const RUST_BUS_TEAM_PREFIX: &str = "github:rust-bus:";

/// One owner, ordered by kind and then id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct OwnerKey {
    /// The [`OwnerKind`] code: 0 for a user, 1 for a team.
    kind: u8,
    /// The user or team id.
    id: u32,
}

impl OwnerKey {
    /// The key of a user id.
    pub(super) fn user(id: u32) -> Self {
        Self {
            kind: OwnerKind::User as u8,
            id,
        }
    }

    /// The key of a team id.
    pub(super) fn team(id: u32) -> Self {
        Self {
            kind: OwnerKind::Team as u8,
            id,
        }
    }

    fn of(owner: Owner) -> Self {
        match owner.kind {
            OwnerKind::User => Self::user(owner.id),
            OwnerKind::Team => Self::team(owner.id),
        }
    }
}

/// A set of owners left out of a crate's user count.
pub(super) type Excluded = HashSet<OwnerKey, FxBuildHasher>;

/// The distinct owners of `crate_id`, in ascending order.
pub(super) fn owner_keys(owners: &IdMap<Vec<Owner>>, crate_id: u32) -> Vec<OwnerKey> {
    let mut keys: Vec<OwnerKey> = owners
        .get(&crate_id)
        .map(|list| list.iter().copied().map(OwnerKey::of).collect())
        .unwrap_or_default();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// The `rust-bus` user and every `github:rust-bus:` team found in the dump.
pub(super) fn rust_bus(users: &IdMap<String>, teams: &IdMap<String>) -> Vec<OwnerKey> {
    let mut keys: Vec<OwnerKey> = users
        .iter()
        .filter(|(_, login)| login.as_str() == RUST_BUS_USER)
        .map(|(&id, _)| OwnerKey::user(id))
        .chain(
            teams
                .iter()
                .filter(|(_, login)| login.starts_with(RUST_BUS_TEAM_PREFIX))
                .map(|(&id, _)| OwnerKey::team(id)),
        )
        .collect();
    keys.sort_unstable();
    keys
}

/// The owners of a target crate, its publishers and the shared accounts.
pub(super) fn excluded(owners: &[OwnerKey], publishers: &[u32], shared: &[OwnerKey]) -> Excluded {
    owners
        .iter()
        .copied()
        .chain(publishers.iter().copied().map(OwnerKey::user))
        .chain(shared.iter().copied())
        .collect()
}

/// The weighted user count of `active`, a list of depender crate ids in ascending order.
///
/// Each depender gives `1 / owners` to each of its owners. Each owner's total
/// is capped at [`OWNER_CAP`]. Excluded owners add nothing. Dependers and
/// owners are summed in ascending order, so the result is the same on every run.
pub(super) fn users(active: &[u32], owners: &IdMap<Vec<OwnerKey>>, excluded: &Excluded) -> f64 {
    let mut totals: BTreeMap<OwnerKey, f64> = BTreeMap::new();
    for depender in active {
        let Some(keys) = owners.get(depender).filter(|keys| !keys.is_empty()) else {
            continue;
        };
        let share = 1.0 / keys.len() as f64;
        for key in keys.iter().filter(|key| !excluded.contains(key)) {
            *totals.entry(*key).or_insert(0.0) += share;
        }
    }
    totals
        .values()
        .fold(0.0, |sum, total| sum + total.min(OWNER_CAP))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owners(rows: Vec<(u32, Vec<OwnerKey>)>) -> IdMap<Vec<OwnerKey>> {
        rows.into_iter().collect()
    }

    #[test]
    fn two_crates_by_one_owner_cap_at_one() {
        let alice = OwnerKey::user(7);
        let map = owners(vec![(1, vec![alice]), (2, vec![alice])]);
        assert_eq!(users(&[1, 2], &map, &Excluded::default()), 1.0);
    }

    #[test]
    fn three_owners_get_a_third_each() {
        let keys = [OwnerKey::user(1), OwnerKey::user(2), OwnerKey::team(3)];
        let map = owners(vec![(1, keys.to_vec())]);
        let excluded: Excluded = [OwnerKey::user(1)].into_iter().collect();
        assert_eq!(users(&[1], &map, &excluded), 1.0 / 3.0 + 1.0 / 3.0);
        assert_eq!(
            users(&[1], &map, &Excluded::default()),
            1.0 / 3.0 + 1.0 / 3.0 + 1.0 / 3.0
        );
    }

    #[test]
    fn partial_shares_add_before_the_cap() {
        let (a, b) = (OwnerKey::user(1), OwnerKey::user(2));
        let map = owners(vec![
            (1, vec![a, b]),
            (2, vec![a]),
            (3, vec![b, OwnerKey::user(3)]),
        ]);
        assert_eq!(
            users(&[1, 2, 3], &map, &Excluded::default()),
            1.0 + 1.0 + 0.5
        );
    }

    #[test]
    fn owners_publishers_and_rust_bus_are_removed() {
        let users_map: IdMap<String> = [(1, "rust-bus".to_owned()), (2, "alice".to_owned())]
            .into_iter()
            .collect();
        let teams_map: IdMap<String> = [
            (5, "github:rust-bus:maintainers".to_owned()),
            (6, "github:other:rust-bus".to_owned()),
        ]
        .into_iter()
        .collect();
        let shared = rust_bus(&users_map, &teams_map);
        assert_eq!(shared, [OwnerKey::user(1), OwnerKey::team(5)]);
        let excluded = excluded(&[OwnerKey::user(2)], &[9], &shared);
        let map = owners(vec![
            (10, vec![OwnerKey::user(2)]),
            (11, vec![OwnerKey::user(9)]),
            (12, vec![OwnerKey::user(1)]),
            (13, vec![OwnerKey::team(5)]),
            (14, vec![OwnerKey::team(6)]),
        ]);
        assert_eq!(users(&[10, 11, 12, 13, 14], &map, &excluded), 1.0);
    }

    #[test]
    fn crates_without_owners_add_nothing() {
        let map = owners(vec![(1, Vec::new())]);
        assert_eq!(users(&[1, 2], &map, &Excluded::default()), 0.0);
    }

    #[test]
    fn owner_keys_are_sorted_and_distinct() {
        let map: IdMap<Vec<Owner>> = [(
            1,
            vec![
                Owner {
                    id: 3,
                    kind: OwnerKind::Team,
                },
                Owner {
                    id: 9,
                    kind: OwnerKind::User,
                },
                Owner {
                    id: 9,
                    kind: OwnerKind::User,
                },
            ],
        )]
        .into_iter()
        .collect();
        assert_eq!(owner_keys(&map, 1), [OwnerKey::user(9), OwnerKey::team(3)]);
        assert!(owner_keys(&map, 2).is_empty());
    }
}
