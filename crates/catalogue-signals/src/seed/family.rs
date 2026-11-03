//! The grouping of eligible crates into families.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use catalogue_core::canonical_crate_name;

use super::fields::repo_key;
use super::types::ExistingIndex;
use crate::discover::Candidate;
use crate::dump::{DependencyKind, DumpSnapshot, IdMap};
use crate::usage::Usage;

/// The eligible crates of one repository.
pub(super) struct Family<'a> {
    /// The top of the family's dependency tree; it names the entry.
    pub(super) primary: &'a Candidate,
    /// The other eligible crates, in tie-breaker order (see [`families`]).
    pub(super) rest: Vec<&'a Candidate>,
    /// Members that fail eligibility, as `(crate name, reason)`.
    pub(super) left_out: Vec<(String, String)>,
}

/// Groups the candidates into families, sorted by primary crate name.
///
/// A candidate joins the pool when no existing entry lists its crate. A pooled
/// crate whose default version is yanked or missing, or that carries a
/// deprecation notice, is not eligible. The pool splits by normalised
/// repository; a crate without an https repository forms a family alone. A
/// family needs at least one eligible member whose active users reach
/// `min_active_users`: the floor applies to the project, so a hub crate with
/// few direct users stays in its family. A group that misses the floor yields
/// no family.
///
/// The primary is the member named like the repository, if any, whether or
/// not it is a root; this covers a hub that plugins depend on. Otherwise it is
/// the top of the family's own dependency tree. The graph has an
/// edge from each eligible member to every other eligible member in its
/// default-version normal or build requirements; dev requirements are
/// ignored. Roots are the members that no other member depends on.
/// - One root is the primary.
/// - Several roots: the tie-breakers below pick among the roots.
/// - No root (a cycle): the tie-breakers pick among all members.
///
/// The canonical name of a member equals the canonical repository name when
/// it matches the last segment of the repository. The ranking, in order: named
/// like the repository; depends on the most family members (the facade); most
/// current active users; shortest name; alphabetical name. The other members
/// follow the primary in that order.
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

    let mut groups: BTreeMap<String, Vec<(&Candidate, f64)>> = BTreeMap::new();
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
    found.sort_by(|a, b| a.primary.name.cmp(&b.primary.name));
    found
}

fn family<'a>(
    snapshot: &DumpSnapshot,
    key: &str,
    members: Vec<(&'a Candidate, f64)>,
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
    if !live.iter().any(|&(_, users)| users >= min_active_users) {
        return None;
    }
    let ids: BTreeSet<u32> = live.iter().map(|(c, _)| c.crate_id).collect();
    let uses = |id: u32| -> BTreeSet<u32> {
        snapshot
            .default_requirements
            .get(&id)
            .into_iter()
            .flatten()
            .filter(|req| req.kind != DependencyKind::Dev)
            .map(|req| req.crate_id)
            .filter(|dep| *dep != id && ids.contains(dep))
            .collect()
    };
    let repo_name = canonical_crate_name(key.rsplit('/').next().unwrap_or_default());
    let mut ranked: Vec<Ranked<'a>> = live
        .into_iter()
        .map(|(candidate, users)| Ranked {
            candidate,
            users,
            named_like_repo: canonical_crate_name(&candidate.name) == repo_name,
            uses: uses(candidate.crate_id),
        })
        .collect();
    let used: BTreeSet<u32> = ranked.iter().flat_map(|r| r.uses.iter().copied()).collect();
    ranked.sort_by(Ranked::order);
    let at = if ranked.first().is_some_and(|r| r.named_like_repo) {
        0
    } else {
        ranked
            .iter()
            .position(|r| !used.contains(&r.candidate.crate_id))
            .unwrap_or_default()
    };
    if ranked.is_empty() {
        return None;
    }
    let primary = ranked.remove(at).candidate;
    Some(Family {
        primary,
        rest: ranked.into_iter().map(|r| r.candidate).collect(),
        left_out,
    })
}

/// An eligible member with the facts the tie-breakers read.
struct Ranked<'a> {
    candidate: &'a Candidate,
    users: f64,
    named_like_repo: bool,
    /// The ids of the other eligible members it depends on.
    uses: BTreeSet<u32>,
}

impl Ranked<'_> {
    /// The tie-breaker order: best first.
    fn order(a: &Self, b: &Self) -> Ordering {
        b.named_like_repo
            .cmp(&a.named_like_repo)
            .then(b.uses.len().cmp(&a.uses.len()))
            .then(b.users.total_cmp(&a.users))
            .then(a.candidate.name.len().cmp(&b.candidate.name.len()))
            .then(a.candidate.name.cmp(&b.candidate.name))
    }
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
    use super::super::fixtures::{Spec, World};
    use super::*;

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

    #[test]
    fn the_facade_wins_although_a_dependency_has_more_users() {
        let found = order(vec![
            member(1, "proj-core", 50.0, &[]),
            member(2, "proj-nn", 20.0, &[1]),
            member(3, "proj-api", 1.0, &[1, 2]),
        ]);
        assert_eq!(found, ["proj-api", "proj-nn", "proj-core"]);
    }

    #[test]
    fn a_hub_named_like_the_repository_wins_over_the_plugins() {
        let found = order(vec![
            member(1, "proj", 5.0, &[]),
            member(2, "proj-a", 50.0, &[1]),
            member(3, "proj-b", 40.0, &[1]),
        ]);
        assert_eq!(found, ["proj", "proj-a", "proj-b"]);
    }

    #[test]
    fn a_single_root_wins_over_every_tie_breaker() {
        let found = order(vec![
            member(1, "base", 99.0, &[]),
            member(2, "z", 1.0, &[1]),
        ]);
        assert_eq!(found, ["z", "base"]);
    }

    #[test]
    fn of_two_roots_the_one_named_like_the_repository_wins() {
        let found = order(vec![
            member(1, "proj-cli", 90.0, &[3, 4]),
            member(2, "Proj", 1.0, &[]),
            member(3, "proj-a", 1.0, &[]),
            member(4, "proj-b", 1.0, &[]),
        ]);
        assert_eq!(found[0], "Proj");
    }

    #[test]
    fn of_two_roots_the_one_with_more_family_dependencies_wins() {
        let found = order(vec![
            member(1, "core", 5.0, &[]),
            member(2, "extra", 5.0, &[]),
            member(3, "a-one", 90.0, &[1]),
            member(4, "b-two", 1.0, &[1, 2]),
        ]);
        assert_eq!(found, ["b-two", "a-one", "core", "extra"]);
    }

    #[test]
    fn roots_tie_on_users_then_shortest_then_alphabetical_name() {
        let found = order(vec![
            member(1, "x-bb", 5.0, &[]),
            member(2, "x-aa", 5.0, &[]),
            member(3, "x-c", 5.0, &[]),
            member(4, "x-top", 9.0, &[]),
        ]);
        assert_eq!(found, ["x-top", "x-c", "x-aa", "x-bb"]);
    }

    #[test]
    fn a_cycle_applies_the_tie_breakers_to_all_members() {
        let found = order(vec![
            member(1, "x-a", 1.0, &[2]),
            member(2, "x-b", 9.0, &[1]),
        ]);
        assert_eq!(found, ["x-b", "x-a"]);
    }

    #[test]
    fn dev_and_outside_dependencies_are_ignored() {
        let mut dev = member(1, "x-a", 1.0, &[]);
        dev.deps = vec![(2, DependencyKind::Dev), (99, DependencyKind::Normal)];
        let found = order(vec![dev, member(2, "x-b", 9.0, &[])]);
        assert_eq!(found, ["x-b", "x-a"]);
    }

    #[test]
    fn a_build_dependency_counts() {
        let mut build = member(1, "x-a", 1.0, &[]);
        build.deps = vec![(2, DependencyKind::Build)];
        let found = order(vec![build, member(2, "x-b", 9.0, &[])]);
        assert_eq!(found, ["x-a", "x-b"]);
    }

    #[test]
    fn a_single_crate_is_its_own_family() {
        assert_eq!(order(vec![member(1, "solo", 3.0, &[])]), ["solo"]);
    }
}
