//! Whether a project binds native code, and whether it has Python bindings.
//!
//! The native code check walks the dependencies of each listed crate's default version. A hit is
//! a `*-sys` crate or a crate with a `links` key. It also reads the build dependencies of the
//! listed crates themselves, for a tool that compiles or links native code. A negative value
//! states only that the check found nothing. It cannot see C built by a script with no `links`
//! key, no `*-sys` crate and no such build tool, or native code kept outside crates.io.
//!
//! The Python check reads the normal dependencies of each listed crate for a Python binding
//! crate. Python bindings that a project keeps outside crates.io stay unseen.

use catalogue_core::{Backends, canonical_crate_name};

use super::family::Member;
use super::native_walk::{Found, build_tools, lists, native_hits};
use super::reach::enabling_feature;
use crate::dump::DumpSnapshot;

/// The value of a measured project where the check finds no native code.
const NO_NATIVE_CODE: &str = "No native code found";

/// The start of the value of a project that binds something.
const BINDS: &str = "Binds native code";

/// The start of the list of build tools in the value.
const BUILDS: &str = "Build uses a native toolchain";

/// The value of a measured project with no Python binding crate on crates.io.
const NO_PYTHON_CRATE: &str = "No Python binding crate found";

/// The start of the value of a project with a Python binding crate.
const HAS_PYTHON: &str = "Has Python bindings";

/// Most names one list in the value names. The rest count as `and N more`.
const MOST_NAMES: usize = 5;

/// The native code value of `members`. `None` without a member.
///
/// The value names each `links` key and each `*-sys` crate that the listed crates reach through
/// normal dependencies, direct ones first. `not_native` in the table lists the crates to skip. A
/// name behind an optional dependency of a listed crate carries the feature that enables it.
/// The build tools that `native_build` lists follow, for the listed crates only.
pub(super) fn native_code(
    members: &[Member<'_>],
    dump: &DumpSnapshot,
    table: &Backends,
) -> Option<String> {
    if members.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    let hits = native_hits(members, dump, table);
    if !hits.is_empty() {
        parts.push(format!("{BINDS}: {}", limited(&hits)));
    }
    let tools = build_tools(members, dump, table);
    if !tools.is_empty() {
        parts.push(format!("{BUILDS}: {}", limited(&tools)));
    }
    Some(if parts.is_empty() {
        NO_NATIVE_CODE.to_owned()
    } else {
        parts.join(". ")
    })
}

/// The first [`MOST_NAMES`] of `found`, then `and N more`.
fn limited(found: &[Found]) -> String {
    let shown: Vec<String> = found
        .iter()
        .take(MOST_NAMES)
        .map(|hit| match &hit.feature {
            Some(feature) => format!("{} (feature {feature})", hit.name),
            None => hit.name.clone(),
        })
        .collect();
    let list = shown.join(", ");
    match found.len().saturating_sub(MOST_NAMES) {
        0 => list,
        more => format!("{list} and {more} more"),
    }
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
            .filter(|dep| lists(&table.python, &canonical_crate_name(dep.name)))
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
        NO_PYTHON_CRATE.to_owned()
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

    const TABLE: &str = "python = [\"pyo3\"]\nnot_native = [\"js-sys\", \"clang-sys\"]\nnative_build = [\"cc\", \"clang-sys\"]\n";

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
        dump.add_crate(14, "cc");
        dump.add_crate(15, "clang-sys");
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
    fn no_binding_reads_as_none_found() {
        let mut dump = dump();
        dump.add_requirement(1, 13, "^1", DependencyKind::Normal);
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("No native code found")
        );
    }

    #[test]
    fn a_sys_crate_below_a_normal_dependency_binds_native_code() {
        let mut dump = dump();
        dump.add_requirement(1, 13, "^1", DependencyKind::Normal);
        dump.add_requirement(13, 10, "^1", DependencyKind::Normal);
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("Binds native code: openssl-sys")
        );
    }

    #[test]
    fn a_sys_crate_behind_an_optional_listed_dependency_names_the_feature() {
        let mut dump = dump();
        dump.add_optional(1, 13);
        dump.add_requirement(13, 10, "^1", DependencyKind::Normal);
        let mut details = CrateDetails::default();
        details
            .features
            .insert("tls".to_owned(), vec!["dep:serde".to_owned()]);
        dump.set_details(1, details);
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("Binds native code: openssl-sys (feature tls)")
        );
    }

    #[test]
    fn a_value_names_at_most_five_and_counts_the_rest() {
        let mut dump = dump();
        for (offset, name) in ["a", "b", "c", "d", "e", "f", "g"].into_iter().enumerate() {
            let id = 30 + u32::try_from(offset).expect("small");
            dump.add_crate(id, &format!("{name}-sys"));
            dump.add_requirement(1, id, "^1", DependencyKind::Normal);
        }
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("Binds native code: a-sys, b-sys, c-sys, d-sys, e-sys and 2 more")
        );
    }

    #[test]
    fn a_build_dependency_on_a_native_tool_shows_the_toolchain() {
        let mut dump = dump();
        dump.add_requirement(1, 14, "^1", DependencyKind::Build);
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("Build uses a native toolchain: cc")
        );
        dump.add_requirement(1, 10, "^1", DependencyKind::Normal);
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("Binds native code: openssl-sys. Build uses a native toolchain: cc")
        );
    }

    #[test]
    fn a_build_tool_alone_is_no_binding_as_a_normal_dependency() {
        let mut dump = dump();
        dump.add_requirement(1, 15, "^1", DependencyKind::Normal);
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("No native code found")
        );
        dump.add_requirement(1, 15, "^1", DependencyKind::Build);
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("Build uses a native toolchain: clang-sys")
        );
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
    fn a_not_native_crate_and_a_dev_dependency_find_nothing() {
        let mut dump = dump();
        dump.add_requirement(1, 11, "^1", DependencyKind::Normal);
        dump.add_requirement(1, 10, "^1", DependencyKind::Dev);
        assert_eq!(
            native(&dump, &["tool"]).as_deref(),
            Some("No native code found")
        );
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
    fn no_python_crate_found_and_an_empty_list_measures_nothing() {
        let dump = dump();
        assert_eq!(
            python(&dump, &["tool"], &table()).as_deref(),
            Some("No Python binding crate found")
        );
        assert_eq!(python(&dump, &["tool"], &Backends::default()), None);
    }
}
