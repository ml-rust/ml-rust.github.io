//! The crates the sandbox check builds.

use catalogue_core::{Catalogue, Date, EntryKind, MaintainerStatus};
use catalogue_signals::assemble::{CrateIndex, context, latest_release};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::history::History;
use catalogue_signals::sandbox::CheckItem;

/// The crates to check, with one note per crate left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxList {
    /// One item per crate release to check, in entry slug order.
    pub items: Vec<CheckItem>,
    /// One line per crate left out.
    pub notes: Vec<String>,
}

/// Every crate, at its latest release, of each library entry that rot signal R6 can decide.
///
/// An entry is listed when the newest snapshot of `history` finds it dormant, or
/// when its latest maintainer statement on `today` is `finished`. Those are the
/// entries whose dead and finished rules read the rot count. Dormant means the
/// tier engine's own verdict, which `catalogue tiers --history` records. Apps
/// have no crate release for R6. A crate without a release in `dump` is noted
/// and left out.
pub fn sandbox_list(
    catalogue: &Catalogue,
    history: &History,
    dump: &DumpSnapshot,
    today: Date,
) -> SandboxList {
    let index = CrateIndex::new(dump);
    let newest = history.snapshots().last();
    let mut listed = SandboxList {
        items: Vec::new(),
        notes: Vec::new(),
    };
    for (slug, entry) in catalogue.entries() {
        if entry.kind != EntryKind::Library {
            continue;
        }
        let dormant = newest
            .and_then(|snapshot| snapshot.rows.get(slug.as_str()))
            .is_some_and(|row| row.dormant == Some(true));
        let finished = context(slug, catalogue, history, today).maintainer_statement
            == Some(MaintainerStatus::Finished);
        if !dormant && !finished {
            continue;
        }
        for name in &entry.crates {
            match latest_release(dump, &index, name.as_str()) {
                Some(release) => listed.items.push(CheckItem {
                    entry: slug.to_string(),
                    name: release.name,
                    version: release.version,
                }),
                None => listed.notes.push(format!(
                    "{slug}: crate {name} has no unyanked release in the dump, skipped"
                )),
            }
        }
    }
    listed
}
