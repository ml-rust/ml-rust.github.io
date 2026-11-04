//! Who depends on a project, from the default versions in one dump.

use crate::dump::{DependencyKind, DumpSnapshot, IdMap, IdSet};

/// The crates whose default version directly depends on each target, built once per check.
///
/// The count differs from active users, which weighs every unyanked version over its lifetime,
/// removes shared owners and takes the most used family crate. Neither one reads the other.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependentIndex {
    dependers: IdMap<Vec<u32>>,
}

impl DependentIndex {
    /// Indexes the normal and build dependencies on `targets`, optional ones included.
    ///
    /// One pass over the default requirements. A dev dependency reaches no user, and a yanked
    /// default version means every version of that crate is yanked. Neither counts.
    pub fn new(dump: &DumpSnapshot, targets: &IdSet) -> Self {
        let mut dependers: IdMap<Vec<u32>> = IdMap::default();
        for (&depender, requirements) in &dump.default_requirements {
            let matches = requirements.iter().filter(|requirement| {
                requirement.kind != DependencyKind::Dev && targets.contains(&requirement.crate_id)
            });
            for requirement in matches {
                let yanked = dump
                    .default_version
                    .get(&depender)
                    .and_then(|version| dump.versions.get(version))
                    .is_some_and(|version| version.yanked);
                if !yanked {
                    dependers
                        .entry(requirement.crate_id)
                        .or_default()
                        .push(depender);
                }
            }
        }
        Self { dependers }
    }

    /// The distinct crates outside `family` that depend on any crate of it.
    ///
    /// `None` for an empty family: the check measured nothing.
    pub fn count(&self, family: &[u32]) -> Option<u64> {
        if family.is_empty() {
            return None;
        }
        let mut found = IdSet::default();
        for target in family {
            if let Some(dependers) = self.dependers.get(target) {
                found.extend(dependers.iter().copied());
            }
        }
        for member in family {
            found.remove(member);
        }
        Some(u64::try_from(found.len()).unwrap_or(u64::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::fixtures::{Dump, day};

    fn dump() -> Dump {
        let mut dump = Dump::new();
        for (id, name) in [(1, "tool"), (2, "tool-extra"), (3, "user"), (4, "other")] {
            dump.add_crate(id, name);
        }
        dump
    }

    fn index(dump: &Dump, targets: &[u32]) -> DependentIndex {
        DependentIndex::new(&dump.snapshot, &targets.iter().copied().collect())
    }

    #[test]
    fn a_crate_that_uses_two_family_crates_counts_once_and_the_family_never_counts() {
        let mut dump = dump();
        dump.add_requirement(3, 1, "^1", DependencyKind::Normal);
        dump.add_requirement(3, 2, "^1", DependencyKind::Normal);
        dump.add_requirement(4, 2, "^1", DependencyKind::Build);
        dump.add_requirement(2, 1, "^1", DependencyKind::Normal);
        let found = index(&dump, &[1, 2]);
        assert_eq!(found.count(&[1, 2]), Some(2));
        assert_eq!(
            found.count(&[1]),
            Some(2),
            "tool-extra counts for tool alone"
        );
    }

    #[test]
    fn a_dev_dependency_never_counts_and_an_optional_one_does() {
        let mut dump = dump();
        dump.add_requirement(3, 1, "^1", DependencyKind::Dev);
        dump.add_optional(4, 1);
        assert_eq!(index(&dump, &[1]).count(&[1]), Some(1));
    }

    #[test]
    fn a_crate_whose_default_version_is_yanked_never_counts() {
        let mut dump = dump();
        dump.add_version(30, 3, "1.0.0", day(2026, 1, 1), true);
        dump.set_default(3, 30);
        dump.add_requirement(3, 1, "^1", DependencyKind::Normal);
        assert_eq!(index(&dump, &[1]).count(&[1]), Some(0));
    }

    #[test]
    fn no_dependent_is_zero_and_no_family_is_not_measured() {
        let found = index(&dump(), &[1]);
        assert_eq!(found.count(&[1]), Some(0));
        assert_eq!(found.count(&[]), None);
    }
}
