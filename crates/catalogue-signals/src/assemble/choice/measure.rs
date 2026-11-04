//! Every choice fact of one entry.

use catalogue_core::{Backends, Entry, EntryKind};

use super::dependents::DependentIndex;
use super::family::members;
use super::license::{Licensed, from_primary, from_repo};
use super::native::{native_code, python_bindings};
use super::runs_on::{RunsOn, runs_on};
use crate::assemble::index::CrateIndex;
use crate::dump::DumpSnapshot;
use crate::github::Contributors;
use crate::history::RepoValues;

/// What every entry of one check shares.
#[derive(Debug, Clone, Copy)]
pub struct ChoiceInputs<'a> {
    /// The crates.io dump, read with the details of every listed crate.
    pub dump: &'a DumpSnapshot,
    /// The name and version index of `dump`.
    pub index: &'a CrateIndex,
    /// The backend table.
    pub backends: &'a Backends,
    /// The dependents of every listed crate.
    pub dependents: &'a DependentIndex,
}

/// The measured choice facts of one entry. `None` means not measured.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChoiceFacts {
    /// The license.
    pub license: Option<Licensed>,
    /// The backends.
    pub runs_on: Option<RunsOn>,
    /// The native code statement.
    pub native_code: Option<String>,
    /// The Python bindings statement.
    pub python: Option<String>,
    /// The distinct crates outside the family that depend on it.
    pub dependents: Option<u64>,
    /// The contributors with a GitHub account.
    pub contributors: Option<u64>,
}

/// The choice facts of `entry`.
///
/// `facts` and `contributors` come from the network, and `None` leaves them not measured.
/// A library reads its license from its primary crate, and falls back to GitHub when the crate
/// states none. An app, or a library whose primary crate the dump does not hold, reads it from
/// GitHub. Every other fact reads every listed crate.
pub fn measure(
    entry: &Entry,
    inputs: &ChoiceInputs<'_>,
    facts: Option<&RepoValues>,
    contributors: Option<&Contributors>,
) -> ChoiceFacts {
    let ChoiceInputs {
        dump,
        index,
        backends,
        dependents,
    } = *inputs;
    let found = members(entry, dump, index);
    let family: Vec<u32> = entry
        .crates
        .iter()
        .filter_map(|name| index.id(name.as_str()))
        .collect();
    let primary = entry
        .crates
        .first()
        .and_then(|name| index.id(name.as_str()))
        .and_then(|id| dump.details.get(&id))
        .filter(|_| entry.kind == EntryKind::Library);
    let repo_license = facts.and_then(RepoValues::license);
    let license = match primary {
        Some(details) => from_primary(details.license.as_deref(), repo_license),
        None => repo_license.and_then(from_repo),
    };
    ChoiceFacts {
        license,
        runs_on: runs_on(&found, dump, backends),
        native_code: native_code(&found, dump, backends),
        python: python_bindings(&found, dump, backends),
        dependents: dependents.count(&family),
        contributors: contributors.and_then(|found| match found {
            Contributors::Counted { count } => Some(*count),
            Contributors::TooLarge => None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::choice::license::LicenseOrigin;
    use crate::assemble::fixtures::{Dump, found_facts, library};
    use crate::dump::{CrateDetails, DependencyKind, IdSet};
    use crate::github::RepoFacts;

    const TABLE: &str =
        "python = [\"pyo3\"]\n[[backend]]\nid = \"cuda\"\nname = \"CUDA\"\nfeatures = [\"cuda\"]\n";

    fn app() -> Entry {
        let text = "name = \"Tool\"\nkind = \"app\"\nrepo = \"https://github.com/ml-rust/tool\"\n\
            crates = [\"tool\"]\ntopics = [\"ml\"]\nadded = 2026-01-01\n\
            [description]\ntext = \"A tool.\"\nsource = \"human\"\n";
        toml::from_str(text).expect("entry parses")
    }

    fn dump() -> Dump {
        let mut dump = Dump::new();
        dump.add_crate(1, "tool");
        dump.add_crate(2, "tool-extra");
        dump.add_crate(3, "user");
        let mut details = CrateDetails {
            license: Some("MIT".to_owned()),
            ..CrateDetails::default()
        };
        details.features.insert("cuda".to_owned(), Vec::new());
        dump.set_details(1, details);
        dump.set_details(2, CrateDetails::default());
        dump.add_requirement(3, 2, "^1", DependencyKind::Normal);
        dump
    }

    fn run(
        dump: &Dump,
        entry: &Entry,
        facts: Option<&RepoValues>,
        contributors: Option<&Contributors>,
    ) -> ChoiceFacts {
        let index = CrateIndex::new(&dump.snapshot);
        let table: Backends = toml::from_str(TABLE).expect("table parses");
        let targets: IdSet = [1, 2].into_iter().collect();
        let dependents = DependentIndex::new(&dump.snapshot, &targets);
        let inputs = ChoiceInputs {
            dump: &dump.snapshot,
            index: &index,
            backends: &table,
            dependents: &dependents,
        };
        measure(entry, &inputs, facts, contributors)
    }

    fn repo_facts() -> RepoValues {
        RepoValues::from(&RepoFacts::Found(found_facts()))
    }

    #[test]
    fn a_family_is_measured_across_every_listed_crate() {
        let dump = dump();
        let found = run(&dump, &library(&["tool", "tool-extra"]), None, None);
        let license = found.license.expect("license");
        assert_eq!(
            (license.text.as_str(), license.origin),
            ("MIT", LicenseOrigin::Dump),
            "the primary crate answers"
        );
        assert_eq!(found.runs_on.expect("runs on").value(), "CUDA");
        assert_eq!(found.native_code.as_deref(), Some("No native code found"));
        assert_eq!(
            found.python.as_deref(),
            Some("No Python binding crate found")
        );
        assert_eq!(
            found.dependents,
            Some(1),
            "user depends on the second crate"
        );
        assert_eq!(found.contributors, None);
    }

    #[test]
    fn a_library_with_no_crate_license_falls_back_to_the_repo() {
        let mut dump = dump();
        dump.set_details(1, CrateDetails::default());
        let facts = repo_facts();
        let entry = library(&["tool"]);
        let license = run(&dump, &entry, Some(&facts), None)
            .license
            .expect("license");
        assert_eq!(
            (license.text.as_str(), license.origin),
            ("MIT", LicenseOrigin::Repo)
        );
        let none = run(&dump, &entry, None, None).license.expect("license");
        assert_eq!(
            (none.text.as_str(), none.origin),
            ("None stated", LicenseOrigin::Dump)
        );
    }

    #[test]
    fn an_app_reads_its_license_from_github_even_with_a_crate() {
        let facts = repo_facts();
        let dump = dump();
        let license = run(&dump, &app(), Some(&facts), None)
            .license
            .expect("license");
        assert_eq!(license.origin, LicenseOrigin::Repo);
        assert_eq!(license.text, "MIT");
    }

    #[test]
    fn a_library_whose_crate_the_dump_lacks_reads_github_and_measures_nothing_else() {
        let facts = repo_facts();
        let dump = dump();
        let found = run(&dump, &library(&["absent"]), Some(&facts), None);
        assert_eq!(found.license.expect("license").origin, LicenseOrigin::Repo);
        assert_eq!(found.runs_on, None);
        assert_eq!(found.native_code, None);
        assert_eq!(found.python, None);
        assert_eq!(found.dependents, None);
    }

    #[test]
    fn a_gone_repo_and_no_fetch_leave_the_license_not_measured() {
        let dump = dump();
        assert_eq!(
            run(&dump, &app(), Some(&RepoValues::Gone), None).license,
            None
        );
        assert_eq!(run(&dump, &app(), None, None).license, None);
    }

    #[test]
    fn a_contributor_list_too_large_is_not_measured() {
        let dump = dump();
        let entry = library(&["tool"]);
        let counted = Contributors::Counted { count: 38 };
        assert_eq!(
            run(&dump, &entry, None, Some(&counted)).contributors,
            Some(38)
        );
        let large = Contributors::TooLarge;
        assert_eq!(run(&dump, &entry, None, Some(&large)).contributors, None);
    }
}
