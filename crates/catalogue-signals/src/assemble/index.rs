//! Name and version lookups over one dump, built once for every entry.

use std::collections::{HashMap, HashSet};

use catalogue_core::canonical_crate_name;

use crate::dump::{DumpSnapshot, IdMap};

/// Crate ids by name, deleted crate names, and version ids by crate id.
///
/// Names use the form crates.io compares: [`canonical_crate_name`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrateIndex {
    by_name: HashMap<String, u32>,
    deleted: HashSet<String>,
    versions: IdMap<Vec<u32>>,
}

impl CrateIndex {
    /// Indexes `dump`. Each crate's version ids come in ascending order.
    pub fn new(dump: &DumpSnapshot) -> Self {
        let by_name = dump
            .crates
            .iter()
            .map(|(&id, row)| (canonical_crate_name(&row.name), id))
            .collect();
        let deleted = dump
            .deleted
            .iter()
            .map(|row| canonical_crate_name(&row.name))
            .collect();
        let mut versions: IdMap<Vec<u32>> = IdMap::default();
        for (&version_id, row) in &dump.versions {
            versions.entry(row.crate_id).or_default().push(version_id);
        }
        for ids in versions.values_mut() {
            ids.sort_unstable();
        }
        Self {
            by_name,
            deleted,
            versions,
        }
    }

    /// The id of crate `name`, in any spelling crates.io treats as equal.
    pub fn id(&self, name: &str) -> Option<u32> {
        self.by_name.get(&canonical_crate_name(name)).copied()
    }

    /// Whether the dump's `deleted_crates` table names crate `name`.
    pub fn is_deleted(&self, name: &str) -> bool {
        self.deleted.contains(&canonical_crate_name(name))
    }

    /// The version ids of crate `crate_id`, ascending. Empty for an unknown crate.
    pub fn versions(&self, crate_id: u32) -> &[u32] {
        self.versions
            .get(&crate_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::fixtures::{Dump, day};

    #[test]
    fn resolves_names_the_crates_io_way() {
        let mut dump = Dump::new();
        dump.add_crate(1, "Foo_Bar");
        dump.add_version(12, 1, "1.0.0", day(2025, 1, 1), false);
        dump.add_version(11, 1, "0.1.0", day(2024, 1, 1), false);
        dump.add_deleted("Gone_Crate");
        let index = CrateIndex::new(&dump.snapshot);
        assert_eq!(index.id("foo-bar"), Some(1));
        assert_eq!(index.id("FOO_BAR"), Some(1));
        assert_eq!(index.id("other"), None);
        assert!(index.is_deleted("gone-crate"));
        assert!(!index.is_deleted("foo-bar"));
        assert_eq!(index.versions(1), [11, 12]);
        assert!(index.versions(9).is_empty());
    }
}
