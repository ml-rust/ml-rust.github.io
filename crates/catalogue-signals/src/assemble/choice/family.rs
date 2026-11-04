//! The listed crates of one entry, as the dump states them.

use catalogue_core::Entry;

use crate::assemble::index::CrateIndex;
use crate::dump::{CrateDetails, DependencyKind, DumpSnapshot, Requirement};

/// One listed crate whose default version the dump holds.
#[derive(Debug, Clone, Copy)]
pub(super) struct Member<'a> {
    /// The crate name as crates.io spells it.
    pub(super) name: &'a str,
    /// The details of the default version.
    pub(super) details: &'a CrateDetails,
    /// The requirements of the default version.
    pub(super) requirements: &'a [Requirement],
}

/// One normal dependency of a [`Member`].
#[derive(Debug, Clone, Copy)]
pub(super) struct Dependency<'a> {
    /// The crate id.
    pub(super) crate_id: u32,
    /// The crate name as crates.io spells it.
    pub(super) name: &'a str,
    /// Whether the dependency is optional.
    pub(super) optional: bool,
}

/// The listed crates of `entry` that have details in `dump`, in entry order.
pub(super) fn members<'a>(
    entry: &Entry,
    dump: &'a DumpSnapshot,
    index: &CrateIndex,
) -> Vec<Member<'a>> {
    entry
        .crates
        .iter()
        .filter_map(|name| {
            let id = index.id(name.as_str())?;
            let row = dump.crates.get(&id)?;
            let details = dump.details.get(&id)?;
            Some(Member {
                name: &row.name,
                details,
                requirements: dump
                    .default_requirements
                    .get(&id)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
            })
        })
        .collect()
}

impl<'a> Member<'a> {
    /// The normal dependencies of the default version whose crate the dump holds.
    ///
    /// A build or dev dependency never reaches the code a user runs.
    pub(super) fn dependencies(
        &self,
        dump: &'a DumpSnapshot,
    ) -> impl Iterator<Item = Dependency<'a>> {
        self.requirements
            .iter()
            .filter(|requirement| requirement.kind == DependencyKind::Normal)
            .filter_map(move |requirement| {
                let row = dump.crates.get(&requirement.crate_id)?;
                Some(Dependency {
                    crate_id: requirement.crate_id,
                    name: &row.name,
                    optional: requirement.optional,
                })
            })
    }
}
