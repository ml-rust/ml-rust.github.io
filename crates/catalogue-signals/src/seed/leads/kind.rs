//! Whether a lead becomes a library or an app.

use catalogue_core::EntryKind;

use super::super::family::Family;
use super::super::primary::words;
use crate::dump::DumpSnapshot;

/// Name segments of a crate that is a program.
const BINARY_SEGMENTS: &[&str] = &["app", "bin", "cli", "daemon", "gui", "server", "tui"];

/// What tells the kind of a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Basis {
    /// No crate claims the repository.
    NoCrate,
    /// The `has_lib` and `bin_names` columns of the dump.
    Measured,
    /// The name rules. A reviewer must check this guess.
    NameGuess,
}

impl Basis {
    /// The label in the draft output.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::NoCrate => "basis:no-crate",
            Self::Measured => "basis:has_lib",
            Self::NameGuess => "basis:name-guess",
        }
    }
}

/// The kind of a draft and what tells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Kind {
    pub(super) entry: EntryKind,
    pub(super) basis: Basis,
}

impl Kind {
    /// The kind of a repository no crate claims.
    pub(super) const NO_CRATE: Self = Self {
        entry: EntryKind::App,
        basis: Basis::NoCrate,
    };
}

/// The targets the dump records for the default version of a crate.
enum Targets {
    Library,
    Binaries,
    Unknown,
}

fn targets(snapshot: &DumpSnapshot, crate_id: u32) -> Targets {
    let version = snapshot
        .default_version
        .get(&crate_id)
        .and_then(|version| snapshot.versions.get(version));
    match version.map(|version| (version.has_lib, version.has_bin)) {
        Some((Some(true), _)) => Targets::Library,
        Some((Some(false), true)) => Targets::Binaries,
        _ => Targets::Unknown,
    }
}

/// The kind of `family`.
///
/// A member with a library target makes a library. A family of binary crates makes an app. The
/// dump leaves a few versions unmarked. The name rules then read each unmarked member: a name
/// with a program segment reads as a binary, and any other name reads as a library.
pub(super) fn classify(snapshot: &DumpSnapshot, family: &Family<'_>) -> Kind {
    let mut unmarked = Vec::new();
    for member in family.members() {
        match targets(snapshot, member.crate_id) {
            Targets::Library => {
                return Kind {
                    entry: EntryKind::Library,
                    basis: Basis::Measured,
                };
            }
            Targets::Binaries => {}
            Targets::Unknown => unmarked.push(member.name.as_str()),
        }
    }
    if unmarked.is_empty() {
        return Kind {
            entry: EntryKind::App,
            basis: Basis::Measured,
        };
    }
    let library = unmarked.iter().any(|name| !names_a_program(name));
    Kind {
        entry: if library {
            EntryKind::Library
        } else {
            EntryKind::App
        },
        basis: Basis::NameGuess,
    }
}

fn names_a_program(name: &str) -> bool {
    words(name)
        .iter()
        .any(|word| BINARY_SEGMENTS.contains(&word.as_str()))
}

#[cfg(test)]
mod tests {
    use super::super::super::family::family;
    use super::super::super::fixtures::{Spec, World};
    use super::*;

    fn spec(id: u32, name: &'static str, has_lib: Option<bool>, has_bin: bool) -> Spec {
        let mut spec = Spec::new(id, name);
        spec.repository = Some("https://github.com/o/proj".to_owned());
        spec.has_lib = has_lib;
        spec.has_bin = has_bin;
        spec
    }

    fn kind(specs: Vec<Spec>) -> (EntryKind, Basis) {
        let world = World::new(specs);
        let members = world.candidates.iter().map(|c| (c, 1.0)).collect();
        let found = family(&world.snapshot, "github.com/o/proj", members, 0.0).expect("one family");
        let kind = classify(&world.snapshot, &found);
        (kind.entry, kind.basis)
    }

    #[test]
    fn one_library_makes_a_library() {
        let found = kind(vec![
            spec(1, "proj", Some(false), true),
            spec(2, "proj-core", Some(true), false),
        ]);
        assert_eq!(found, (EntryKind::Library, Basis::Measured));
    }

    #[test]
    fn a_library_with_a_binary_stays_a_library() {
        let found = kind(vec![spec(1, "proj", Some(true), true)]);
        assert_eq!(found, (EntryKind::Library, Basis::Measured));
    }

    #[test]
    fn only_binaries_make_an_app() {
        let found = kind(vec![
            spec(1, "proj", Some(false), true),
            spec(2, "proj-extra", Some(false), true),
        ]);
        assert_eq!(found, (EntryKind::App, Basis::Measured));
    }

    #[test]
    fn an_unmarked_version_falls_back_to_a_labelled_guess() {
        assert_eq!(
            kind(vec![spec(1, "proj", None, false)]),
            (EntryKind::Library, Basis::NameGuess)
        );
        assert_eq!(
            kind(vec![spec(1, "proj-cli", None, false)]),
            (EntryKind::App, Basis::NameGuess)
        );
    }

    #[test]
    fn no_library_and_no_binary_is_unmarked() {
        assert_eq!(
            kind(vec![spec(1, "proj", Some(false), false)]),
            (EntryKind::Library, Basis::NameGuess)
        );
    }
}
