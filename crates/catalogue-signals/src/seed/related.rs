//! The evidence that a draft belongs to a bigger project in another repo.
//!
//! A name alone proves nothing: anyone can publish a crate under any free name. Crate owners
//! are set by crates.io, so a shared owner carries the evidence that the name only suggests.

use std::collections::{HashMap, HashSet};

use catalogue_core::{Entry, Slug, canonical_crate_name};

use super::types::ExistingIndex;
use crate::dump::{DumpSnapshot, Owner};

/// A possible parent: its slug and the canonical name of its primary crate.
type Parent = (Slug, String);

/// Sets `part_of` on each draft with evidence for it.
///
/// The evidence for a parent has two parts. The draft's primary crate name starts with the
/// parent's primary crate name plus `-` or `_`. The two crates share a crate owner in the
/// dump. A crate with no owner row shares no owner. The primary crate is `crates[0]`.
///
/// The longest matching name wins. A parent has no `part_of` of its own, so shorter names
/// settle first and an entry already placed under a parent never becomes one.
pub(super) fn set_part_of(
    snapshot: &DumpSnapshot,
    existing: &ExistingIndex,
    drafts: &mut [(Slug, Entry)],
) {
    let mut order: Vec<usize> = (0..drafts.len()).collect();
    order.sort_by_key(|&at| primary_name(&drafts[at].1).map(|name| name.len()));
    let mut parents: Vec<Parent> = existing.parents().to_vec();
    let owners = owners_by_name(snapshot, drafts, &parents);
    for at in order {
        let (slug, entry) = &mut drafts[at];
        let Some(name) = primary_name(entry) else {
            continue;
        };
        match parent_of(&name, &parents, &owners) {
            Some(parent) => entry.part_of = Some(parent),
            None => parents.push((slug.clone(), name)),
        }
    }
}

/// The canonical name of the primary crate of `entry`.
fn primary_name(entry: &Entry) -> Option<String> {
    let primary = entry.crates.first()?;
    Some(canonical_crate_name(primary.as_str()))
}

/// The parent with the longest name that `name` extends and that shares an owner with it.
fn parent_of(
    name: &str,
    parents: &[Parent],
    owners: &HashMap<String, HashSet<Owner>>,
) -> Option<Slug> {
    let held = owners.get(name)?;
    parents
        .iter()
        .filter(|(_, parent)| extends(name, parent))
        .filter(|(_, parent)| {
            owners
                .get(parent)
                .is_some_and(|theirs| !held.is_disjoint(theirs))
        })
        .max_by_key(|(_, parent)| parent.len())
        .map(|(slug, _)| slug.clone())
}

/// Whether the canonical `name` is the canonical `parent` plus `-` and more.
///
/// The canonical form reads `_` as `-`, so this covers both separators.
fn extends(name: &str, parent: &str) -> bool {
    name.strip_prefix(parent)
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|rest| !rest.is_empty())
}

/// The owners of each primary crate of `drafts` and `parents`, by canonical name.
fn owners_by_name(
    snapshot: &DumpSnapshot,
    drafts: &[(Slug, Entry)],
    parents: &[Parent],
) -> HashMap<String, HashSet<Owner>> {
    let wanted: HashSet<String> = drafts
        .iter()
        .filter_map(|(_, entry)| primary_name(entry))
        .chain(parents.iter().map(|(_, name)| name.clone()))
        .collect();
    let mut owners = HashMap::new();
    for (id, row) in &snapshot.crates {
        let name = canonical_crate_name(&row.name);
        if wanted.contains(&name) {
            let held: HashSet<Owner> = snapshot
                .owners
                .get(id)
                .into_iter()
                .flatten()
                .copied()
                .collect();
            owners.insert(name, held);
        }
    }
    owners
}

#[cfg(test)]
mod tests {
    use catalogue_core::CrateName;

    use super::super::fixtures::{Spec, World, team, today, user};
    use super::super::types::Mappings;
    use super::super::{draft::draft_entries, fixtures};
    use super::*;

    fn owned(id: u32, name: &'static str, owners: Vec<Owner>) -> Spec {
        let mut spec = Spec::new(id, name);
        spec.owners = owners;
        spec
    }

    fn drafts(world: &World, existing: &ExistingIndex) -> Vec<(Slug, Entry)> {
        draft_entries(
            &world.snapshot,
            &world.candidates,
            &world.usage,
            &Mappings {
                topics: &fixtures::topics(),
                categories: &fixtures::categories(),
            },
            existing,
            0.0,
            today(),
        )
        .entries
    }

    fn part_of<'a>(entries: &'a [(Slug, Entry)], slug: &str) -> Option<&'a str> {
        entries
            .iter()
            .find(|(found, _)| found.as_str() == slug)
            .and_then(|(_, entry)| entry.part_of.as_ref())
            .map(Slug::as_str)
    }

    fn slug(text: &str) -> Slug {
        Slug::new(text).expect("slug")
    }

    #[test]
    fn a_name_and_a_shared_owner_set_the_parent() {
        let world = World::new(vec![
            owned(1, "numr", vec![user(1)]),
            owned(2, "numr-audio", vec![user(1), user(2)]),
            owned(3, "numr_video", vec![user(1)]),
        ]);
        let found = drafts(&world, &ExistingIndex::default());
        assert_eq!(part_of(&found, "numr"), None);
        assert_eq!(part_of(&found, "numr-audio"), Some("numr"));
        assert_eq!(part_of(&found, "numr-video"), Some("numr"));
    }

    #[test]
    fn a_shared_team_counts_as_a_shared_owner() {
        let world = World::new(vec![
            owned(1, "numr", vec![team(5)]),
            owned(2, "numr-audio", vec![team(5)]),
        ]);
        let found = drafts(&world, &ExistingIndex::default());
        assert_eq!(part_of(&found, "numr-audio"), Some("numr"));
    }

    #[test]
    fn a_name_alone_sets_nothing() {
        let world = World::new(vec![
            owned(1, "numr", vec![user(1)]),
            owned(2, "numr-audio", vec![user(2)]),
        ]);
        let found = drafts(&world, &ExistingIndex::default());
        assert_eq!(found.len(), 2);
        assert_eq!(part_of(&found, "numr-audio"), None);
    }

    #[test]
    fn a_crate_without_an_owner_row_is_no_evidence() {
        let mut world = World::new(vec![
            owned(1, "numr", Vec::new()),
            owned(2, "numr-audio", Vec::new()),
        ]);
        world.snapshot.owners.clear();
        let found = drafts(&world, &ExistingIndex::default());
        assert_eq!(part_of(&found, "numr-audio"), None);
    }

    #[test]
    fn a_shared_owner_alone_sets_nothing() {
        let world = World::new(vec![
            owned(1, "numr", vec![user(1)]),
            owned(2, "solvr", vec![user(1)]),
            owned(3, "numrs", vec![user(1)]),
        ]);
        let found = drafts(&world, &ExistingIndex::default());
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|(_, entry)| entry.part_of.is_none()));
    }

    #[test]
    fn an_existing_entry_can_be_the_parent() {
        let mut world = World::new(vec![
            owned(1, "numr", vec![user(1)]),
            owned(2, "numr-audio", vec![user(1)]),
        ]);
        world.candidates.retain(|found| found.name == "numr-audio");
        let existing =
            ExistingIndex::new(["numr"], ["numr-lib"]).with_parent(slug("numr-lib"), "numr");
        let found = drafts(&world, &existing);
        assert_eq!(part_of(&found, "numr-audio"), Some("numr-lib"));
    }

    #[test]
    fn a_placed_entry_never_becomes_a_parent() {
        let world = World::new(vec![
            owned(1, "numr", vec![user(1)]),
            owned(2, "numr-audio", vec![user(1)]),
            owned(3, "numr-audio-mel", vec![user(1)]),
        ]);
        let found = drafts(&world, &ExistingIndex::default());
        assert_eq!(part_of(&found, "numr-audio"), Some("numr"));
        assert_eq!(part_of(&found, "numr-audio-mel"), Some("numr"));
    }

    #[test]
    fn the_longest_parent_wins_when_it_has_no_parent_itself() {
        let world = World::new(vec![
            owned(1, "numr", vec![user(1)]),
            owned(2, "numr-audio", vec![user(2)]),
            owned(3, "numr-audio-mel", vec![user(1), user(2)]),
        ]);
        let found = drafts(&world, &ExistingIndex::default());
        assert_eq!(part_of(&found, "numr-audio"), None);
        assert_eq!(part_of(&found, "numr-audio-mel"), Some("numr-audio"));
    }

    #[test]
    fn a_draft_without_a_crate_stays_apart() {
        let world = World::new(vec![owned(1, "numr", vec![user(1)])]);
        let mut found = drafts(&world, &ExistingIndex::default());
        let (_, entry) = found.first_mut().expect("one draft");
        let mut app = entry.clone();
        app.crates = Vec::<CrateName>::new();
        let mut entries = vec![(slug("numr-app"), app), (slug("numr"), entry.clone())];
        set_part_of(&world.snapshot, &ExistingIndex::default(), &mut entries);
        assert!(entries.iter().all(|(_, entry)| entry.part_of.is_none()));
    }

    #[test]
    fn extends_needs_a_separator_and_more_text() {
        assert!(extends("numr-audio", "numr"));
        assert!(!extends("numrs", "numr"));
        assert!(!extends("numr", "numr"));
        assert!(!extends("numr-", "numr"));
    }
}
