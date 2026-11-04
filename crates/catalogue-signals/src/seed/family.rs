//! The grouping of eligible crates into families.

use std::collections::BTreeMap;

use super::fields::repo_key;
use super::owners::{Member, builds_on_primary, owner_groups};
use super::primary::{RepoName, main_group, rank};
use super::types::ExistingIndex;
use crate::discover::Candidate;
use crate::dump::{DumpSnapshot, IdMap};
use crate::usage::Usage;

/// The eligible crates of one project.
pub(super) struct Family<'a> {
    /// The main crate of the repository: `crates[0]` of the draft.
    pub(super) primary: &'a Candidate,
    /// The other members, the crates most like the primary first (see [`rank`]).
    pub(super) rest: Vec<&'a Candidate>,
    /// Crates that claim the repository and stay out, as `(crate name, reason)`.
    pub(super) left_out: Vec<(String, String)>,
    /// The most active users on one member.
    pub(super) users: f64,
}

impl<'a> Family<'a> {
    /// The primary, then the other members.
    pub(super) fn members(&self) -> impl Iterator<Item = &'a Candidate> {
        std::iter::once(self.primary).chain(self.rest.iter().copied())
    }
}

/// Groups the candidates into families, the most used family first, then by primary crate name.
///
/// A candidate joins the pool when no existing entry lists its crate. A pooled
/// crate whose default version is yanked or missing, or that carries a
/// deprecation notice, is not eligible. The pool splits by normalised
/// repository; a crate without an https repository forms a family alone.
///
/// The repository link alone does not make a crate a member, because any publisher can claim
/// any link. The claimants split by shared owners (see [`owner_groups`]). The members are the
/// group that holds the project (see [`main_group`]), and any other crate that builds on the
/// primary under the same first word (see [`builds_on_primary`]). The others are left out with
/// the reason.
///
/// The primary is the first member in [`rank`] order. A family needs at least one member whose
/// active users reach `min_active_users`: the floor applies to the project, so a hub crate
/// with few direct users stays in its family, and a fork with users does not lift it. A group
/// that misses the floor yields no family.
pub(super) fn families<'a>(
    snapshot: &DumpSnapshot,
    candidates: &'a [Candidate],
    usage: &IdMap<Usage>,
    existing: &ExistingIndex,
    min_active_users: f64,
) -> Vec<Family<'a>> {
    let mut sorted: Vec<&Candidate> = candidates.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name).then(a.crate_id.cmp(&b.crate_id)));
    sorted.dedup_by_key(|candidate| candidate.crate_id);

    let mut groups: BTreeMap<String, Vec<Member<'a>>> = BTreeMap::new();
    for candidate in sorted {
        let users = usage.get(&candidate.crate_id).map_or(0.0, |u| u.current);
        if existing.lists_crate(&candidate.name) {
            continue;
        }
        let raw = snapshot
            .crates
            .get(&candidate.crate_id)
            .and_then(|row| row.repository.as_deref());
        let key = repo_key(raw).unwrap_or_else(|| format!("crate:{}", candidate.name));
        groups.entry(key).or_default().push((candidate, users));
    }
    let mut found: Vec<Family<'a>> = groups
        .into_iter()
        .filter_map(|(key, members)| family(snapshot, &key, members, min_active_users))
        .collect();
    found.sort_by(|a, b| {
        b.users
            .total_cmp(&a.users)
            .then_with(|| a.primary.name.cmp(&b.primary.name))
    });
    found
}

/// The family of the crates `members` that claim the repository `key`, by the rules of [`families`].
pub(super) fn family<'a>(
    snapshot: &DumpSnapshot,
    key: &str,
    members: Vec<Member<'a>>,
    min_active_users: f64,
) -> Option<Family<'a>> {
    let mut live = Vec::new();
    let mut left_out = Vec::new();
    for (candidate, users) in members {
        match unfit_reason(snapshot, candidate) {
            Some(reason) => left_out.push((candidate.name.clone(), reason.to_owned())),
            None => live.push((candidate, users)),
        }
    }
    let repo = RepoName::new(key.rsplit('/').next().unwrap_or_default());
    let (main, strays) = main_group(&repo, owner_groups(snapshot, live))?;
    let mut users = main.iter().map(|(_, users)| *users).fold(0.0, f64::max);
    let mut ranked = rank(snapshot, &repo, &main).into_iter();
    let primary = ranked.next()?;
    let mut rest: Vec<&'a Candidate> = ranked.collect();
    for (stray, stray_users) in strays {
        if builds_on_primary(snapshot, stray, primary) {
            rest.push(stray);
            users = users.max(stray_users);
        } else {
            left_out.push((
                stray.name.clone(),
                format!(
                    "shares no owner with `{}`, so left out of the family",
                    primary.name
                ),
            ));
        }
    }
    (users >= min_active_users).then_some(Family {
        primary,
        rest,
        left_out,
        users,
    })
}

/// Why a pooled crate is not eligible, or `None` when it is.
fn unfit_reason(snapshot: &DumpSnapshot, candidate: &Candidate) -> Option<&'static str> {
    if candidate.deprecated {
        return Some("deprecated, so left out of the family");
    }
    let live = snapshot
        .default_version
        .get(&candidate.crate_id)
        .and_then(|version| snapshot.versions.get(version))
        .is_some_and(|version| !version.yanked);
    (!live).then_some("default version is yanked or missing, so left out of the family")
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{Spec, World, user};
    use super::*;
    use crate::dump::{DependencyKind, Owner};

    fn owned(mut spec: Spec, owners: &[Owner]) -> Spec {
        spec.owners = owners.to_vec();
        spec
    }

    fn member(id: u32, name: &'static str, users: f64, deps: &[u32]) -> Spec {
        let mut spec = Spec::new(id, name);
        spec.repository = Some("https://github.com/o/proj".to_owned());
        spec.users = users;
        spec.deps = deps
            .iter()
            .map(|id| (*id, DependencyKind::Normal))
            .collect();
        spec
    }

    /// The primary, then the rest, of the only family.
    fn order(specs: Vec<Spec>) -> Vec<String> {
        let world = World::new(specs);
        let found = families(
            &world.snapshot,
            &world.candidates,
            &world.usage,
            &ExistingIndex::default(),
            0.0,
        );
        let [family] = found.as_slice() else {
            panic!("one family expected");
        };
        std::iter::once(family.primary)
            .chain(family.rest.iter().copied())
            .map(|c| c.name.clone())
            .collect()
    }

    /// The members and the left-out crates of the only family, at a floor of `min`.
    fn split(specs: Vec<Spec>, min: f64) -> Option<(Vec<String>, Vec<String>)> {
        let world = World::new(specs);
        let found = families(
            &world.snapshot,
            &world.candidates,
            &world.usage,
            &ExistingIndex::default(),
            min,
        );
        let family = found.first()?;
        let members = std::iter::once(family.primary)
            .chain(family.rest.iter().copied())
            .map(|c| c.name.clone())
            .collect();
        let mut left_out: Vec<String> = family.left_out.iter().map(|(n, _)| n.clone()).collect();
        left_out.sort();
        Some((members, left_out))
    }

    #[test]
    fn the_core_crate_wins_although_an_examples_crate_tops_the_tree() {
        let found = order(vec![
            member(1, "proj-core", 50.0, &[]),
            member(2, "proj-nn", 20.0, &[1]),
            member(3, "proj-examples", 1.0, &[1, 2]),
        ]);
        assert_eq!(found, ["proj-core", "proj-nn", "proj-examples"]);
    }

    #[test]
    fn a_single_crate_is_its_own_family() {
        assert_eq!(order(vec![member(1, "solo", 3.0, &[])]), ["solo"]);
    }

    #[test]
    fn foreign_crates_that_claim_the_repository_are_left_out() {
        let found = split(
            vec![
                member(1, "proj-core", 50.0, &[]),
                member(2, "proj-nn", 20.0, &[1]),
                owned(member(3, "onde-proj-core", 1.0, &[]), &[user(7)]),
                owned(member(4, "onde-proj-nn", 1.0, &[3]), &[user(7)]),
                owned(member(5, "proj-core-temp", 0.0, &[]), &[user(8)]),
            ],
            0.0,
        );
        let (members, left_out) = found.expect("one family");
        assert_eq!(members, ["proj-core", "proj-nn"]);
        assert_eq!(
            left_out,
            ["onde-proj-core", "onde-proj-nn", "proj-core-temp"]
        );
    }

    #[test]
    fn a_fork_with_a_deep_tree_does_not_become_the_primary() {
        let found = split(
            vec![
                member(1, "proj", 18.0, &[2]),
                member(2, "proj-core", 2.0, &[]),
                owned(member(3, "fork-a", 0.0, &[]), &[user(7)]),
                owned(member(4, "fork-b", 0.0, &[3]), &[user(7)]),
                owned(member(5, "fork-c", 0.0, &[3, 4]), &[user(7)]),
            ],
            0.0,
        );
        let (members, _) = found.expect("one family");
        assert_eq!(members, ["proj", "proj-core"]);
    }

    #[test]
    fn the_group_named_like_the_repository_wins_over_a_group_with_more_users() {
        let found = split(
            vec![
                member(1, "proj", 2.0, &[]),
                owned(member(2, "fork", 90.0, &[]), &[user(7)]),
            ],
            0.0,
        );
        assert_eq!(
            found,
            Some((vec!["proj".to_owned()], vec!["fork".to_owned()]))
        );
    }

    #[test]
    fn without_a_name_match_the_group_with_more_users_wins() {
        let found = split(
            vec![
                owned(member(1, "a-tool", 1.0, &[]), &[user(7)]),
                member(2, "z-core", 90.0, &[]),
            ],
            0.0,
        );
        assert_eq!(
            found,
            Some((vec!["z-core".to_owned()], vec!["a-tool".to_owned()]))
        );
    }

    #[test]
    fn a_shared_owner_keeps_a_crate_with_another_name() {
        let found = split(
            vec![
                member(1, "proj-core", 50.0, &[]),
                member(2, "tensor-tools", 0.0, &[1]),
                member(3, "pytorch-reader", 1.0, &[]),
            ],
            0.0,
        );
        let (members, left_out) = found.expect("one family");
        assert_eq!(members[0], "proj-core");
        assert_eq!(members.len(), 3);
        assert!(left_out.is_empty(), "{left_out:?}");
    }

    #[test]
    fn a_contributor_crate_on_the_primary_stays_after_the_project_crates() {
        let found = split(
            vec![
                member(1, "proj", 5.0, &[]),
                owned(member(2, "proj-extra", 9.0, &[1]), &[user(7)]),
                member(3, "proj-nn", 1.0, &[1]),
            ],
            0.0,
        );
        let (members, left_out) = found.expect("one family");
        assert_eq!(members, ["proj", "proj-nn", "proj-extra"]);
        assert!(left_out.is_empty(), "{left_out:?}");
    }

    #[test]
    fn the_floor_applies_to_the_project_and_not_to_a_fork() {
        let specs = || {
            vec![
                member(1, "proj", 1.0, &[]),
                owned(member(2, "fork", 90.0, &[]), &[user(7)]),
            ]
        };
        assert_eq!(split(specs(), 2.0), None);
        assert!(split(specs(), 1.0).is_some());
    }

    #[test]
    fn the_floor_counts_any_member_of_the_project() {
        let found = split(
            vec![member(1, "proj", 0.0, &[]), member(2, "proj-nn", 5.0, &[1])],
            2.0,
        );
        assert!(found.is_some());
    }

    #[test]
    fn families_sort_by_users_then_primary_name() {
        let repo = |mut spec: Spec, name: &str| {
            spec.repository = Some(format!("https://github.com/o/{name}"));
            spec
        };
        let world = World::new(vec![
            repo(member(1, "alpha", 1.0, &[]), "alpha"),
            repo(member(2, "zeta", 9.0, &[]), "zeta"),
            repo(member(3, "beta", 1.0, &[]), "beta"),
        ]);
        let found = families(
            &world.snapshot,
            &world.candidates,
            &world.usage,
            &ExistingIndex::default(),
            0.0,
        );
        let names: Vec<&str> = found.iter().map(|f| f.primary.name.as_str()).collect();
        assert_eq!(names, ["zeta", "alpha", "beta"]);
    }
}
