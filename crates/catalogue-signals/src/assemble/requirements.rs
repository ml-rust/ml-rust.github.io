//! The deprecated requirement of a crate's default version.

use catalogue_core::{Deprecated, RequirementCheck};

use crate::dump::{DependencyKind, DumpSnapshot};
use crate::rustsec::Advisories;

/// The page that publishes each RustSec advisory, by id.
const ADVISORY_URL: &str = "https://rustsec.org/advisories";

/// The outcome of checking a crate's default-version requirements against the list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Found {
    /// The first deprecated requirement in dump order, with its source link. A
    /// curated match reads `<crate> <req> (<source>)`. An unmaintained advisory
    /// reads `<crate> (<RUSTSEC-ID>, <advisory URL>)`.
    pub first: Option<String>,
    /// Requirements on listed crates that the reader does not parse. They count neither way.
    pub unparsed: u32,
}

/// Checks the normal and build requirements of crate `crate_id`'s default version.
///
/// A requirement is deprecated by one of two sources. The curated `deprecated`
/// list names it with a source link, or RustSec holds a non-withdrawn
/// unmaintained advisory on the required crate. The curated list is checked
/// first for each requirement. The first match in dump order wins.
///
/// Optional requirements count: they ship in the release. Dev requirements do
/// not. A requirement on a crate id the dump does not name is skipped.
pub(super) fn deprecated_requirement(
    dump: &DumpSnapshot,
    crate_id: u32,
    deprecated: &Deprecated,
    advisories: &Advisories,
) -> Found {
    let mut found = Found::default();
    let requirements = dump
        .default_requirements
        .get(&crate_id)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for requirement in requirements {
        if requirement.kind == DependencyKind::Dev {
            continue;
        }
        let Some(row) = dump.crates.get(&requirement.crate_id) else {
            continue;
        };
        let curated = match deprecated.check(&row.name, &requirement.req) {
            RequirementCheck::Deprecated => deprecated
                .find(&row.name)
                .map(|listed| format!("{} {} ({})", row.name, requirement.req, listed.source)),
            RequirementCheck::Unparsed => {
                found.unparsed = found.unparsed.saturating_add(1);
                None
            }
            RequirementCheck::Allowed => None,
        };
        let matched = curated.or_else(|| {
            advisories.unmaintained(&row.name).map(|advisory| {
                format!(
                    "{} ({}, {ADVISORY_URL}/{}.html)",
                    row.name, advisory.id, advisory.id
                )
            })
        });
        if found.first.is_none() {
            found.first = matched;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::fixtures::{Dump, day};
    use crate::rustsec::{Advisory, AdvisoryKind};

    fn list() -> Deprecated {
        toml::from_str(
            "[[requirement]]\ncrate = \"tokio\"\nbelow = \"1.0.0\"\nreason = \"r\"\nsource = \"https://example.com/r\"\n\
             [[requirement]]\ncrate = \"failure\"\nbelow = \"999999.0.0\"\nreason = \"r\"\nsource = \"https://example.com/r\"\n",
        )
        .expect("list parses")
    }

    fn dump() -> Dump {
        let mut dump = Dump::new();
        dump.add_crate(1, "lib");
        dump.add_crate(2, "tokio");
        dump.add_crate(3, "failure");
        dump.add_crate(4, "serde");
        dump
    }

    #[test]
    fn finds_the_first_deprecated_normal_or_build_requirement() {
        let mut dump = dump();
        dump.add_requirement(1, 4, "^1", DependencyKind::Normal);
        dump.add_requirement(1, 2, "0.1", DependencyKind::Dev);
        dump.add_requirement(1, 3, "0.1.8", DependencyKind::Build);
        dump.add_requirement(1, 2, "^0.2", DependencyKind::Normal);
        let found = deprecated_requirement(&dump.snapshot, 1, &list(), &Advisories::default());
        assert_eq!(
            found.first.as_deref(),
            Some("failure 0.1.8 (https://example.com/r)")
        );
        assert_eq!(found.unparsed, 0);
    }

    #[test]
    fn allowed_requirements_find_nothing() {
        let mut dump = dump();
        dump.add_requirement(1, 2, "1", DependencyKind::Normal);
        dump.add_requirement(1, 4, "0.1", DependencyKind::Normal);
        dump.add_requirement(1, 9, "0.1", DependencyKind::Normal);
        assert_eq!(
            deprecated_requirement(&dump.snapshot, 1, &list(), &Advisories::default()),
            Found::default()
        );
        assert_eq!(
            deprecated_requirement(&dump.snapshot, 7, &list(), &Advisories::default()),
            Found::default()
        );
    }

    #[test]
    fn unparsed_requirements_on_listed_crates_are_counted() {
        let mut dump = dump();
        dump.add_requirement(1, 2, "~>0.1", DependencyKind::Normal);
        dump.add_requirement(1, 4, "~>0.1", DependencyKind::Normal);
        let found = deprecated_requirement(&dump.snapshot, 1, &list(), &Advisories::default());
        assert_eq!(found.first, None);
        assert_eq!(found.unparsed, 1);
    }

    fn advisories(package: &str, id: &str, withdrawn: bool) -> Advisories {
        let advisory = Advisory {
            id: id.to_owned(),
            package: package.to_owned(),
            date: day(2024, 1, 1),
            kind: AdvisoryKind::Unmaintained,
            withdrawn: withdrawn.then(|| day(2025, 1, 1)),
            patched: Vec::new(),
            unaffected: Vec::new(),
            url: None,
        };
        let mut all = Advisories::default();
        all.by_crate.insert(package.to_owned(), vec![advisory]);
        all
    }

    #[test]
    fn an_unmaintained_advisory_marks_the_requirement_with_its_id() {
        let mut dump = dump();
        dump.add_crate(5, "old-dep");
        dump.add_requirement(1, 4, "^1", DependencyKind::Normal);
        dump.add_requirement(1, 5, "^2", DependencyKind::Build);
        let all = advisories("old-dep", "RUSTSEC-2024-0001", false);
        let found = deprecated_requirement(&dump.snapshot, 1, &list(), &all);
        assert_eq!(
            found.first.as_deref(),
            Some(
                "old-dep (RUSTSEC-2024-0001, https://rustsec.org/advisories/RUSTSEC-2024-0001.html)"
            )
        );
    }

    #[test]
    fn the_curated_list_wins_over_an_advisory_on_the_same_requirement() {
        let mut dump = dump();
        dump.add_requirement(1, 3, "0.1.8", DependencyKind::Normal);
        let all = advisories("failure", "RUSTSEC-2020-0036", false);
        let found = deprecated_requirement(&dump.snapshot, 1, &list(), &all);
        assert_eq!(
            found.first.as_deref(),
            Some("failure 0.1.8 (https://example.com/r)")
        );
    }

    #[test]
    fn a_withdrawn_advisory_and_a_dev_requirement_find_nothing() {
        let mut dump = dump();
        dump.add_crate(5, "old-dep");
        dump.add_crate(6, "dev-dep");
        dump.add_requirement(1, 5, "^2", DependencyKind::Normal);
        dump.add_requirement(1, 6, "^1", DependencyKind::Dev);
        let mut all = advisories("old-dep", "RUSTSEC-2024-0001", true);
        all.by_crate
            .extend(advisories("dev-dep", "RUSTSEC-2024-0002", false).by_crate);
        assert_eq!(
            deprecated_requirement(&dump.snapshot, 1, &list(), &all),
            Found::default()
        );
    }
}
