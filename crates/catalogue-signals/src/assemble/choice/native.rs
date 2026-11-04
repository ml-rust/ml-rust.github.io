//! Whether a project binds native code, and whether it has Python bindings.
//!
//! These read each listed crate and its direct dependencies. A deeper dependency stays unseen,
//! and so does C built with no `links` key and no `*-sys` dependency. Listing a crate on an
//! entry is what brings its dependencies into view.

use std::collections::BTreeMap;

use catalogue_core::{Backends, CrateName, canonical_crate_name};

use super::family::Member;
use super::reach::enabling_feature;
use crate::dump::DumpSnapshot;

/// The value of a measured project that binds nothing.
const PURE_RUST: &str = "Pure Rust";

/// The start of the value of a project that binds something.
const BINDS: &str = "Binds native code";

/// The value of a measured project with no Python binding crate.
const NO_PYTHON: &str = "No Python bindings";

/// The start of the value of a project with a Python binding crate.
const HAS_PYTHON: &str = "Has Python bindings";

/// The ending of a crate name that binds a foreign library by convention.
const SYS_SUFFIX: &str = "-sys";

/// Whether `list` names `canonical`, a canonical crate name.
fn names(list: &[CrateName], canonical: &str) -> bool {
    list.iter().any(|name| name.normalized() == canonical)
}

/// The native code value of `members`. `None` without a member.
///
/// The value names each `links` key and each `*-sys` crate, listed or a direct normal dependency.
/// `not_native` in the table lists the crates to skip. An optional dependency names the feature
/// that enables it.
pub(super) fn native_code(
    members: &[Member<'_>],
    dump: &DumpSnapshot,
    table: &Backends,
) -> Option<String> {
    if members.is_empty() {
        return None;
    }
    let binds = |name: &str| {
        let canonical = canonical_crate_name(name);
        canonical.ends_with(SYS_SUFFIX) && !names(&table.not_native, &canonical)
    };
    let mut found: BTreeMap<String, Option<String>> = BTreeMap::new();
    for member in members {
        if let Some(links) = &member.details.links {
            found.insert(links.clone(), None);
        }
        if binds(member.name) {
            found.insert(member.name.to_owned(), None);
        }
        for dependency in member.dependencies(dump).filter(|dep| binds(dep.name)) {
            if !dependency.optional {
                found.insert(dependency.name.to_owned(), None);
            } else if !found.contains_key(dependency.name) {
                let feature = enabling_feature(member, &dependency, &[]);
                found.insert(dependency.name.to_owned(), Some(feature));
            }
        }
    }
    if found.is_empty() {
        return Some(PURE_RUST.to_owned());
    }
    let list: Vec<String> = found
        .into_iter()
        .map(|(name, feature)| match feature {
            Some(feature) => format!("{name} (feature {feature})"),
            None => name,
        })
        .collect();
    Some(format!("{BINDS}: {}", list.join(", ")))
}

/// The Python bindings value of `members`. `None` without a member or without a `python` list.
///
/// A required dependency needs Python for that crate. An optional one needs it only with the
/// feature that enables it.
pub(super) fn python_bindings(
    members: &[Member<'_>],
    dump: &DumpSnapshot,
    table: &Backends,
) -> Option<String> {
    if members.is_empty() || table.python.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    for member in members {
        let matched: Vec<_> = member
            .dependencies(dump)
            .filter(|dep| names(&table.python, &canonical_crate_name(dep.name)))
            .collect();
        if matched.iter().any(|dependency| !dependency.optional) {
            lines.push(format!("{} needs Python", member.name));
        } else if let Some(optional) = matched.first() {
            let feature = enabling_feature(member, optional, &[]);
            lines.push(format!(
                "{} needs Python only with feature {feature}",
                member.name
            ));
        }
    }
    Some(if lines.is_empty() {
        NO_PYTHON.to_owned()
    } else {
        format!("{HAS_PYTHON}: {}", lines.join(", "))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::choice::family::members;
    use crate::assemble::fixtures::{Dump, library};
    use crate::assemble::index::CrateIndex;
    use crate::dump::{CrateDetails, DependencyKind};

    const TABLE: &str = "python = [\"pyo3\"]\nnot_native = [\"js-sys\"]\n";

    fn table() -> Backends {
        toml::from_str(TABLE).expect("table parses")
    }

    fn dump() -> Dump {
        let mut dump = Dump::new();
        dump.add_crate(1, "tool");
        dump.add_crate(2, "tool-py");
        dump.add_crate(10, "openssl-sys");
        dump.add_crate(11, "js-sys");
        dump.add_crate(12, "pyo3");
        dump.add_crate(13, "serde");
        dump.set_details(1, CrateDetails::default());
        dump
    }

    fn native(dump: &Dump, crates: &[&str]) -> Option<String> {
        let index = CrateIndex::new(&dump.snapshot);
        let entry = library(crates);
        native_code(
            &members(&entry, &dump.snapshot, &index),
            &dump.snapshot,
            &table(),
        )
    }

    fn python(dump: &Dump, crates: &[&str], table: &Backends) -> Option<String> {
        let index = CrateIndex::new(&dump.snapshot);
        let entry = library(crates);
        python_bindings(
            &members(&entry, &dump.snapshot, &index),
            &dump.snapshot,
            table,
        )
    }

    #[test]
    fn no_binding_reads_as_pure_rust() {
        let mut dump = dump();
        dump.add_requirement(1, 13, "^1", DependencyKind::Normal);
        assert_eq!(native(&dump, &["tool"]).as_deref(), Some("Pure Rust"));
    }

    #[test]
    fn a_sys_dependency_binds_native_code() {
        let mut dump = dump();
        dump.add_requirement(1, 10, "^1", DependencyKind::Normal);
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("Binds native code: openssl-sys")
        );
    }

    #[test]
    fn a_links_crate_binds_native_code() {
        let mut dump = dump();
        dump.set_details(
            1,
            CrateDetails {
                links: Some("torch".to_owned()),
                ..CrateDetails::default()
            },
        );
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("Binds native code: torch")
        );
    }

    #[test]
    fn a_listed_sys_crate_binds_native_code() {
        let mut dump = dump();
        dump.set_details(10, CrateDetails::default());
        assert_eq!(
            native(&dump, &["openssl-sys"]).as_deref(),
            Some("Binds native code: openssl-sys")
        );
    }

    #[test]
    fn an_optional_sys_dependency_names_its_feature() {
        let mut dump = dump();
        dump.add_optional(1, 10);
        let mut details = CrateDetails::default();
        details
            .features
            .insert("tls".to_owned(), vec!["dep:openssl-sys".to_owned()]);
        dump.set_details(1, details);
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("Binds native code: openssl-sys (feature tls)")
        );
    }

    #[test]
    fn a_required_use_replaces_an_optional_one_across_the_family() {
        let mut dump = dump();
        dump.set_details(2, CrateDetails::default());
        dump.add_optional(1, 10);
        dump.add_requirement(2, 10, "^1", DependencyKind::Normal);
        assert_eq!(
            native(&dump, &["tool", "tool-py"]).as_deref(),
            Some("Binds native code: openssl-sys")
        );
    }

    #[test]
    fn a_not_native_crate_and_a_dev_dependency_stay_pure_rust() {
        let mut dump = dump();
        dump.add_requirement(1, 11, "^1", DependencyKind::Normal);
        dump.add_requirement(1, 10, "^1", DependencyKind::Dev);
        assert_eq!(native(&dump, &["tool"]).as_deref(), Some("Pure Rust"));
    }

    #[test]
    fn no_details_read_as_not_measured() {
        assert_eq!(native(&dump(), &["serde"]), None);
    }

    #[test]
    fn a_required_python_dependency_needs_python() {
        let mut dump = dump();
        dump.add_requirement(1, 12, "^1", DependencyKind::Normal);
        assert_eq!(
            python(&dump, &["tool"], &table()).as_deref(),
            Some("Has Python bindings: tool needs Python")
        );
    }

    #[test]
    fn an_optional_python_dependency_needs_python_only_with_its_feature() {
        let mut dump = dump();
        dump.add_optional(1, 12);
        let mut details = CrateDetails::default();
        details
            .features
            .insert("python".to_owned(), vec!["dep:pyo3".to_owned()]);
        dump.set_details(1, details);
        assert_eq!(
            python(&dump, &["tool"], &table()).as_deref(),
            Some("Has Python bindings: tool needs Python only with feature python")
        );
    }

    #[test]
    fn each_family_crate_answers_for_itself() {
        let mut dump = dump();
        dump.set_details(2, CrateDetails::default());
        dump.add_requirement(2, 12, "^1", DependencyKind::Normal);
        assert_eq!(
            python(&dump, &["tool", "tool-py"], &table()).as_deref(),
            Some("Has Python bindings: tool-py needs Python")
        );
    }

    #[test]
    fn no_python_crate_reads_no_and_an_empty_list_measures_nothing() {
        let dump = dump();
        assert_eq!(
            python(&dump, &["tool"], &table()).as_deref(),
            Some("No Python bindings")
        );
        assert_eq!(python(&dump, &["tool"], &Backends::default()), None);
    }
}
