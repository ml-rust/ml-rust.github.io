//! The evidence that a crate belongs to the project whose repository it names.
//!
//! Any publisher can name any repository in a crate's metadata. Forks do: they copy the crates of
//! a project and keep its link. Crate owners are set by crates.io, so they carry the evidence.

use std::collections::HashSet;

use super::primary::lead_word;
use crate::discover::Candidate;
use crate::dump::{DependencyKind, DumpSnapshot, Owner};

/// A crate with its current active users.
pub(super) type Member<'a> = (&'a Candidate, f64);

/// Splits `members` into groups joined by shared owners.
///
/// Two crates join when they share a user or team owner, directly or through other members.
/// A crate without an owner row stays alone.
pub(super) fn owner_groups<'a>(
    snapshot: &DumpSnapshot,
    members: Vec<Member<'a>>,
) -> Vec<Vec<Member<'a>>> {
    let mut groups: Vec<(HashSet<Owner>, Vec<Member<'a>>)> = Vec::new();
    for member in members {
        let mut owners: HashSet<Owner> = snapshot
            .owners
            .get(&member.0.crate_id)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        let mut crates = vec![member];
        let (joined, apart): (Vec<_>, Vec<_>) = groups
            .into_iter()
            .partition(|(held, _)| !held.is_disjoint(&owners));
        groups = apart;
        for (held, their_crates) in joined {
            owners.extend(held);
            crates.extend(their_crates);
        }
        groups.push((owners, crates));
    }
    groups.into_iter().map(|(_, crates)| crates).collect()
}

/// Whether `stray` shares no owner with the project yet builds on it under the project's name.
///
/// A contributor can publish a family crate under their own account. Such a crate depends on the
/// primary. A fork copies the primary and does not depend on it.
pub(super) fn builds_on_primary(
    snapshot: &DumpSnapshot,
    stray: &Candidate,
    primary: &Candidate,
) -> bool {
    let depends = snapshot
        .default_requirements
        .get(&stray.crate_id)
        .into_iter()
        .flatten()
        .any(|req| req.crate_id == primary.crate_id && req.kind != DependencyKind::Dev);
    depends && lead_word(&stray.name) == lead_word(&primary.name)
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{Spec, World, team, user};
    use super::*;

    fn names(world: &World) -> Vec<Vec<String>> {
        let members: Vec<Member<'_>> = world.candidates.iter().map(|c| (c, 0.0)).collect();
        let mut groups: Vec<Vec<String>> = owner_groups(&world.snapshot, members)
            .into_iter()
            .map(|group| {
                let mut found: Vec<String> = group.iter().map(|(c, _)| c.name.clone()).collect();
                found.sort();
                found
            })
            .collect();
        groups.sort();
        groups
    }

    fn owned(id: u32, name: &'static str, owners: Vec<Owner>) -> Spec {
        let mut spec = Spec::new(id, name);
        spec.owners = owners;
        spec
    }

    #[test]
    fn a_crate_of_another_owner_stands_apart() {
        let world = World::new(vec![
            owned(1, "candle-core", vec![user(1), user(2)]),
            owned(2, "candle-nn", vec![user(2)]),
            owned(3, "onde-candle-core", vec![user(9)]),
        ]);
        assert_eq!(
            names(&world),
            [vec!["candle-core", "candle-nn"], vec!["onde-candle-core"]]
        );
    }

    #[test]
    fn a_shared_team_joins_crates() {
        let world = World::new(vec![
            owned(1, "linfa", vec![user(1), team(5)]),
            owned(2, "linfa-nn", vec![user(2), team(5)]),
        ]);
        assert_eq!(names(&world), [vec!["linfa", "linfa-nn"]]);
    }

    #[test]
    fn a_user_and_a_team_with_one_id_do_not_join() {
        let world = World::new(vec![
            owned(1, "a", vec![user(5)]),
            owned(2, "b", vec![team(5)]),
        ]);
        assert_eq!(names(&world), [vec!["a"], vec!["b"]]);
    }

    #[test]
    fn owners_join_through_a_third_crate_in_any_order() {
        let world = World::new(vec![
            owned(1, "a", vec![user(1)]),
            owned(2, "b", vec![user(2)]),
            owned(3, "c", vec![user(1), user(2)]),
        ]);
        assert_eq!(names(&world), [vec!["a", "b", "c"]]);
    }

    #[test]
    fn crates_without_an_owner_row_stay_alone() {
        let world = World::new(vec![owned(1, "a", Vec::new()), owned(2, "b", Vec::new())]);
        assert_eq!(names(&world), [vec!["a"], vec!["b"]]);
    }

    #[test]
    fn a_contributor_crate_builds_on_the_primary() {
        let mut stray = owned(2, "linfa-ensemble", vec![user(9)]);
        stray.deps = vec![(1, DependencyKind::Normal)];
        let world = World::new(vec![owned(1, "linfa", vec![user(1)]), stray]);
        let [primary, stray] = world.candidates.as_slice() else {
            panic!("two candidates expected");
        };
        assert!(builds_on_primary(&world.snapshot, stray, primary));
    }

    #[test]
    fn a_dependency_alone_or_a_shared_word_alone_is_no_evidence() {
        let mut other_word = owned(2, "extra-thing", vec![user(9)]);
        other_word.deps = vec![(1, DependencyKind::Normal)];
        let no_dependency = owned(3, "proj-core-temp", vec![user(9)]);
        let mut dev_only = owned(4, "proj-dev", vec![user(9)]);
        dev_only.deps = vec![(1, DependencyKind::Dev)];
        let world = World::new(vec![
            owned(1, "proj-core", vec![user(1)]),
            other_word,
            no_dependency,
            dev_only,
        ]);
        let [primary, strays @ ..] = world.candidates.as_slice() else {
            panic!("candidates expected");
        };
        for stray in strays {
            assert!(
                !builds_on_primary(&world.snapshot, stray, primary),
                "{}",
                stray.name
            );
        }
    }
}
