//! The native code that the dependencies of the listed crates reach.
//!
//! The walk starts at the listed crates and follows the required normal dependencies of each
//! crate's default version, then theirs in turn. It also follows the optional normal
//! dependencies of a listed crate, and labels what it finds there with the enabling feature.
//! It never follows an optional dependency below a listed crate. A hit is a `*-sys` crate or a
//! crate with a `links` key, unless the table lists it as not native.
//!
//! The walk reads the default version only. It cannot see C built by a script with no `links`
//! key and no `*-sys` crate, or native code kept outside crates.io.

use std::collections::BTreeMap;

use catalogue_core::{Backends, CrateName, canonical_crate_name};

use super::family::Member;
use super::reach::enabling_feature;
use crate::dump::{DependencyKind, DumpSnapshot, IdSet};

/// The ending of a crate name that binds a foreign library by convention.
const SYS_SUFFIX: &str = "-sys";

/// Whether `list` names `canonical`, a canonical crate name.
pub(super) fn lists(list: &[CrateName], canonical: &str) -> bool {
    list.iter()
        .any(|name| same_canonical(name.as_str().as_bytes(), canonical.as_bytes()))
}

/// Whether the crate name `raw` folds to the canonical bytes `canonical`, without a copy.
fn same_canonical(raw: &[u8], canonical: &[u8]) -> bool {
    raw.len() == canonical.len()
        && raw.iter().zip(canonical).all(|(byte, &want)| {
            let folded = if *byte == b'_' {
                b'-'
            } else {
                byte.to_ascii_lowercase()
            };
            folded == want
        })
}

/// Whether the crate name `raw` ends in [`SYS_SUFFIX`] once folded, without a copy.
fn ends_with_sys(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    bytes.len() >= SYS_SUFFIX.len()
        && same_canonical(
            &bytes[bytes.len() - SYS_SUFFIX.len()..],
            SYS_SUFFIX.as_bytes(),
        )
}

/// One name the walk found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Found {
    /// The crate name, or the `links` key of a listed crate.
    pub(super) name: String,
    /// The feature that enables it. `None` when a required path reaches it.
    pub(super) feature: Option<String>,
}

/// What the walk knows of one name.
struct Slot {
    feature: Option<String>,
    direct: bool,
}

/// The names found so far.
#[derive(Default)]
struct Finds(BTreeMap<String, Slot>);

impl Finds {
    /// Records `name`. A required path replaces a feature label, and a direct path sticks.
    fn add(&mut self, name: &str, feature: Option<&str>, direct: bool) {
        let slot = self.0.entry(name.to_owned()).or_insert_with(|| Slot {
            feature: feature.map(str::to_owned),
            direct,
        });
        slot.direct |= direct;
        if feature.is_none() {
            slot.feature = None;
        }
    }

    /// The names with the direct ones first, each group sorted.
    fn ordered(self) -> Vec<Found> {
        let (direct, deeper): (Vec<_>, Vec<_>) =
            self.0.into_iter().partition(|(_, slot)| slot.direct);
        direct
            .into_iter()
            .chain(deeper)
            .map(|(name, slot)| Found {
                name,
                feature: slot.feature,
            })
            .collect()
    }
}

/// The state of one walk.
struct Walk<'a> {
    dump: &'a DumpSnapshot,
    table: &'a Backends,
    visited: IdSet,
    finds: Finds,
}

impl<'a> Walk<'a> {
    /// Whether the crate `name`, with or without a `links` key, binds native code.
    fn binds(&self, name: &str, has_links: bool) -> bool {
        (has_links || ends_with_sys(name))
            && !lists(&self.table.not_native, &canonical_crate_name(name))
    }

    /// Marks crate `crate_id` seen and records it when it binds native code.
    /// Returns `false` when an earlier visit saw it.
    fn visit(&mut self, crate_id: u32, feature: Option<&str>, direct: bool) -> bool {
        if !self.visited.insert(crate_id) {
            return false;
        }
        if let Some(row) = self.dump.crates.get(&crate_id) {
            let has_links = self
                .dump
                .details
                .get(&crate_id)
                .is_some_and(|details| details.links.is_some());
            if self.binds(&row.name, has_links) {
                self.finds.add(&row.name, feature, direct);
            }
        }
        true
    }

    /// Follows the required normal dependencies below the crates in `queue`.
    fn descend(&mut self, mut queue: Vec<u32>, feature: Option<&str>) {
        let dump = self.dump;
        while let Some(crate_id) = queue.pop() {
            let Some(requirements) = dump.default_requirements.get(&crate_id) else {
                continue;
            };
            for requirement in requirements {
                let below = requirement.crate_id;
                let required = requirement.kind == DependencyKind::Normal && !requirement.optional;
                if required && dump.crates.contains_key(&below) && self.visit(below, feature, false)
                {
                    queue.push(below);
                }
            }
        }
    }
}

/// The native code that `members` reach, direct hits first, then the rest, each sorted.
pub(super) fn native_hits(
    members: &[Member<'_>],
    dump: &DumpSnapshot,
    table: &Backends,
) -> Vec<Found> {
    let mut walk = Walk {
        dump,
        table,
        visited: members.iter().map(|member| member.crate_id).collect(),
        finds: Finds::default(),
    };
    for member in members {
        let listed_skipped = lists(&table.not_native, &canonical_crate_name(member.name));
        if let Some(links) = member.details.links.as_deref().filter(|_| !listed_skipped) {
            walk.finds.add(links, None, true);
        }
        if walk.binds(member.name, false) {
            walk.finds.add(member.name, None, true);
        }
    }
    let mut queue = Vec::new();
    for member in members {
        for dependency in member.dependencies(dump).filter(|dep| !dep.optional) {
            if walk.visit(dependency.crate_id, None, true) {
                queue.push(dependency.crate_id);
            }
        }
    }
    walk.descend(queue, None);
    for member in members {
        for dependency in member.dependencies(dump).filter(|dep| dep.optional) {
            let feature = enabling_feature(member, &dependency, &[]);
            if walk.visit(dependency.crate_id, Some(&feature), true) {
                walk.descend(vec![dependency.crate_id], Some(&feature));
            }
        }
    }
    walk.finds.ordered()
}

/// The build tools that `members` name as build dependencies, each sorted.
///
/// Only the listed crates count. A tool behind an optional build dependency names the feature.
pub(super) fn build_tools(
    members: &[Member<'_>],
    dump: &DumpSnapshot,
    table: &Backends,
) -> Vec<Found> {
    let mut finds = Finds::default();
    for member in members {
        for dependency in member.build_dependencies(dump) {
            if !lists(&table.native_build, &canonical_crate_name(dependency.name)) {
                continue;
            }
            let feature = dependency
                .optional
                .then(|| enabling_feature(member, &dependency, &[]));
            finds.add(dependency.name, feature.as_deref(), true);
        }
    }
    finds.ordered()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::choice::family::members;
    use crate::assemble::fixtures::{Dump, library};
    use crate::assemble::index::CrateIndex;
    use crate::dump::CrateDetails;

    const TABLE: &str = "not_native = [\"clang-sys\"]\nnative_build = [\"cc\", \"clang-sys\"]\n";

    fn table() -> Backends {
        toml::from_str(TABLE).expect("table parses")
    }

    /// Crate 1 is `tool` with details. Crates 10 and up are the crates it can reach.
    fn dump() -> Dump {
        let mut dump = Dump::new();
        dump.add_crate(1, "tool");
        dump.add_crate(10, "mid");
        dump.add_crate(11, "deep");
        dump.add_crate(12, "deep-sys");
        dump.add_crate(13, "other-sys");
        dump.add_crate(14, "cc");
        dump.add_crate(15, "clang-sys");
        dump.add_crate(16, "linker");
        dump.set_details(1, CrateDetails::default());
        dump
    }

    fn normal(dump: &mut Dump, from: u32, on: u32) {
        dump.add_requirement(from, on, "^1", DependencyKind::Normal);
    }

    fn hits(dump: &Dump) -> Vec<(String, Option<String>)> {
        let index = CrateIndex::new(&dump.snapshot);
        let entry = library(&["tool"]);
        let found = members(&entry, &dump.snapshot, &index);
        native_hits(&found, &dump.snapshot, &table())
            .into_iter()
            .map(|hit| (hit.name, hit.feature))
            .collect()
    }

    fn names(dump: &Dump) -> Vec<String> {
        hits(dump).into_iter().map(|(name, _)| name).collect()
    }

    fn tools(dump: &Dump) -> Vec<(String, Option<String>)> {
        let index = CrateIndex::new(&dump.snapshot);
        let entry = library(&["tool"]);
        let found = members(&entry, &dump.snapshot, &index);
        build_tools(&found, &dump.snapshot, &table())
            .into_iter()
            .map(|hit| (hit.name, hit.feature))
            .collect()
    }

    fn with_links(dump: &mut Dump, crate_id: u32, links: &str) {
        dump.set_details(
            crate_id,
            CrateDetails {
                links: Some(links.to_owned()),
                ..CrateDetails::default()
            },
        );
    }

    fn feature(name: &str, enables: &str) -> CrateDetails {
        let mut details = CrateDetails::default();
        details
            .features
            .insert(name.to_owned(), vec![format!("dep:{enables}")]);
        details
    }

    #[test]
    fn a_hit_below_a_normal_dependency_is_found() {
        let mut dump = dump();
        normal(&mut dump, 1, 10);
        normal(&mut dump, 10, 11);
        normal(&mut dump, 11, 12);
        assert_eq!(hits(&dump), [("deep-sys".to_owned(), None)]);
    }

    #[test]
    fn a_links_key_on_a_deeper_crate_is_a_hit() {
        let mut dump = dump();
        normal(&mut dump, 1, 10);
        normal(&mut dump, 10, 11);
        with_links(&mut dump, 11, "zlib");
        assert_eq!(hits(&dump), [("deep".to_owned(), None)]);
    }

    #[test]
    fn an_optional_dependency_of_a_listed_crate_names_its_feature_down_the_path() {
        let mut dump = dump();
        dump.set_details(1, feature("onnx", "mid"));
        dump.add_optional(1, 10);
        normal(&mut dump, 10, 12);
        assert_eq!(
            hits(&dump),
            [("deep-sys".to_owned(), Some("onnx".to_owned()))]
        );
    }

    #[test]
    fn an_optional_dependency_below_a_listed_crate_is_not_followed() {
        let mut dump = dump();
        normal(&mut dump, 1, 10);
        dump.add_optional(10, 12);
        assert!(hits(&dump).is_empty());
    }

    #[test]
    fn a_required_path_wins_over_an_optional_one() {
        let mut dump = dump();
        dump.set_details(1, feature("onnx", "mid"));
        dump.add_optional(1, 10);
        normal(&mut dump, 10, 12);
        normal(&mut dump, 1, 11);
        normal(&mut dump, 11, 12);
        assert_eq!(hits(&dump), [("deep-sys".to_owned(), None)]);
    }

    #[test]
    fn a_cycle_ends() {
        let mut dump = dump();
        normal(&mut dump, 1, 10);
        normal(&mut dump, 10, 11);
        normal(&mut dump, 11, 10);
        normal(&mut dump, 11, 1);
        normal(&mut dump, 11, 12);
        assert_eq!(names(&dump), ["deep-sys"]);
    }

    #[test]
    fn a_direct_hit_comes_before_a_deeper_one() {
        let mut dump = dump();
        normal(&mut dump, 1, 10);
        normal(&mut dump, 10, 12);
        normal(&mut dump, 1, 13);
        assert_eq!(names(&dump), ["other-sys", "deep-sys"]);
    }

    #[test]
    fn a_not_native_crate_is_no_hit_even_with_a_links_key() {
        let mut dump = dump();
        normal(&mut dump, 1, 15);
        with_links(&mut dump, 15, "clang");
        assert!(hits(&dump).is_empty());
    }

    #[test]
    fn a_dev_dependency_is_not_followed() {
        let mut dump = dump();
        dump.add_requirement(1, 12, "^1", DependencyKind::Dev);
        assert!(hits(&dump).is_empty());
    }

    #[test]
    fn a_build_dependency_on_a_listed_tool_shows_the_toolchain() {
        let mut dump = dump();
        dump.add_requirement(1, 14, "^1", DependencyKind::Build);
        dump.add_requirement(1, 16, "^1", DependencyKind::Build);
        assert_eq!(tools(&dump), [("cc".to_owned(), None)]);
    }

    #[test]
    fn a_normal_dependency_on_a_build_tool_is_no_toolchain() {
        let mut dump = dump();
        normal(&mut dump, 1, 14);
        assert!(tools(&dump).is_empty());
    }

    #[test]
    fn an_optional_build_tool_names_its_feature() {
        let mut dump = dump();
        dump.set_details(1, feature("bundled", "cc"));
        dump.snapshot
            .default_requirements
            .entry(1)
            .or_default()
            .push(crate::dump::Requirement {
                req: "^1".to_owned(),
                crate_id: 14,
                kind: DependencyKind::Build,
                optional: true,
            });
        assert_eq!(
            tools(&dump),
            [("cc".to_owned(), Some("bundled".to_owned()))]
        );
    }
}
