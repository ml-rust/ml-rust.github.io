//! The repo link signal and the release it checks.

use catalogue_core::{Entry, EntryKind, Signals};

use super::index::CrateIndex;
use super::versions::primary_releases;
use crate::dump::DumpSnapshot;
use crate::link::{CrateRelease, LinkCheck};

/// The primary crate of `entry` at its latest unyanked version, the release the link check reads.
///
/// Returns `None` when the entry lists no crate, or [`latest_release`] finds none.
pub fn primary_release(
    entry: &Entry,
    dump: &DumpSnapshot,
    index: &CrateIndex,
) -> Option<CrateRelease> {
    latest_release(dump, index, entry.crates.first()?.as_str())
}

/// Crate `name` at its latest unyanked version, under the name the dump stores.
///
/// The version follows `latest_version`. Returns `None` when the dump lacks
/// the crate or the crate has no unyanked version.
pub fn latest_release(dump: &DumpSnapshot, index: &CrateIndex, name: &str) -> Option<CrateRelease> {
    let id = index.id(name)?;
    let row = dump.crates.get(&id)?;
    let mut found = Signals::unknown(EntryKind::Library);
    primary_releases(dump, index, id, &mut found);
    Some(CrateRelease {
        name: row.name.clone(),
        version: found.latest_version?,
    })
}

/// Sets `repo_link_verified` from `link` and adds a note for each result that
/// leaves the link unproven or unchecked.
///
/// `None` leaves the signal unknown and adds no note.
pub(super) fn apply_link(link: Option<&LinkCheck>, signals: &mut Signals, notes: &mut Vec<String>) {
    let Some(check) = link else {
        return;
    };
    signals.repo_link_verified = check.verified();
    match check {
        LinkCheck::Verified { .. } => {}
        LinkCheck::CommitMissing { sha1, .. } => {
            notes.push(format!("release commit {sha1} is absent from the repo"));
        }
        LinkCheck::NotOnDefaultBranch { sha1, .. } => {
            notes.push(format!(
                "published from commit {sha1} that the default branch does not contain"
            ));
        }
        LinkCheck::NoVcsInfo => notes.push(
            "the latest release records no commit in .cargo_vcs_info.json: repo link unverified"
                .to_owned(),
        ),
        LinkCheck::UnsupportedHost => {
            notes.push("repo link unchecked: the repo is not on GitHub".to_owned());
        }
    }
    if check.dirty() {
        notes.push("the latest release was packaged from a dirty working tree".to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::fixtures::{World, day, library};

    const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";

    fn linked(check: &LinkCheck) -> (Option<bool>, Vec<String>) {
        let mut world = World::new();
        world.dump.add_crate(1, "main");
        let assembled = world.run_linked(&["main"], Some(check));
        (assembled.signals.repo_link_verified, assembled.notes)
    }

    #[test]
    fn a_found_commit_verifies_the_link_without_a_note() {
        let check = LinkCheck::Verified {
            sha1: SHA.to_owned(),
            dirty: false,
        };
        assert_eq!(linked(&check), (Some(true), Vec::new()));
    }

    #[test]
    fn a_dirty_release_is_checked_and_noted() {
        let check = LinkCheck::Verified {
            sha1: SHA.to_owned(),
            dirty: true,
        };
        let (verified, notes) = linked(&check);
        assert_eq!(verified, Some(true));
        assert_eq!(
            notes,
            ["the latest release was packaged from a dirty working tree"]
        );
    }

    #[test]
    fn a_missing_commit_fails_the_link_and_names_the_commit() {
        let check = LinkCheck::CommitMissing {
            sha1: SHA.to_owned(),
            dirty: true,
        };
        let (verified, notes) = linked(&check);
        assert_eq!(verified, Some(false));
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes[0].contains(SHA), "{notes:?}");
    }

    #[test]
    fn a_commit_off_the_default_branch_fails_the_link_and_names_the_commit() {
        let check = LinkCheck::NotOnDefaultBranch {
            sha1: SHA.to_owned(),
            dirty: false,
        };
        let (verified, notes) = linked(&check);
        assert_eq!(verified, Some(false));
        assert_eq!(
            notes,
            [format!(
                "published from commit {SHA} that the default branch does not contain"
            )]
        );
    }

    #[test]
    fn a_release_without_commit_data_fails_the_link_with_a_note() {
        let (verified, notes) = linked(&LinkCheck::NoVcsInfo);
        assert_eq!(verified, Some(false));
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains(".cargo_vcs_info.json"), "{notes:?}");
    }

    #[test]
    fn an_unsupported_host_leaves_the_link_unknown_with_a_note() {
        let (verified, notes) = linked(&LinkCheck::UnsupportedHost);
        assert_eq!(verified, None);
        assert_eq!(notes, ["repo link unchecked: the repo is not on GitHub"]);
    }

    #[test]
    fn no_result_leaves_the_link_unknown_and_silent() {
        let mut world = World::new();
        world.dump.add_crate(1, "main");
        let assembled = world.run_linked(&["main"], None);
        assert_eq!(assembled.signals.repo_link_verified, None);
        assert!(assembled.notes.is_empty());
    }

    #[test]
    fn the_release_is_the_default_unyanked_version_under_the_dump_name() {
        let mut world = World::new();
        world.dump.add_crate(1, "Main_Crate");
        world
            .dump
            .add_version(10, 1, "1.0.0", day(2024, 1, 1), false);
        world
            .dump
            .add_version(11, 1, "1.1.0", day(2024, 2, 1), false);
        world
            .dump
            .add_version(12, 1, "1.2.0", day(2024, 3, 1), true);
        let index = CrateIndex::new(&world.dump.snapshot);
        let release = primary_release(&library(&["main-crate"]), &world.dump.snapshot, &index);
        assert_eq!(
            release,
            Some(CrateRelease {
                name: "Main_Crate".to_owned(),
                version: "1.1.0".to_owned(),
            })
        );
    }

    #[test]
    fn no_release_without_a_crate_a_dump_row_or_an_unyanked_version() {
        let mut world = World::new();
        world.dump.add_crate(1, "main");
        world
            .dump
            .add_version(10, 1, "1.0.0", day(2024, 1, 1), true);
        let index = CrateIndex::new(&world.dump.snapshot);
        let dump = &world.dump.snapshot;
        assert_eq!(primary_release(&library(&["main"]), dump, &index), None);
        assert_eq!(primary_release(&library(&["absent"]), dump, &index), None);
        assert_eq!(primary_release(&library(&[]), dump, &index), None);
    }
}
