use serde::{Deserialize, Serialize};

use crate::ids::{CrateName, Slug};
use crate::report::{NO_GPU_FEATURE, NOT_MEASURED, RUNS_ON_SEPARATOR};

/// The tag id of the value [`NO_GPU_FEATURE`]. A table row cannot use it.
pub const NO_GPU_FEATURE_ID: &str = "gpu-unknown";

/// The tag id of the value [`NOT_MEASURED`]. A table row cannot use it.
pub const NOT_MEASURED_ID: &str = "not-measured";

/// Contents of `data/backends.toml`: the names that show what a project runs on.
///
/// An absent file reads as an empty table, and every fact that reads it stays not measured.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backends {
    /// Crates whose presence as a dependency shows Python bindings.
    #[serde(default)]
    pub python: Vec<CrateName>,
    /// Crates named `*-sys` that bind no foreign library. The native code fact skips them.
    #[serde(default)]
    pub not_native: Vec<CrateName>,
    /// Crates whose presence as a build dependency of a listed crate shows that its build
    /// compiles or links native code.
    #[serde(default)]
    pub native_build: Vec<CrateName>,
    /// The `[[backend]]` tables. The order sets the order of the `runs_on` fact.
    #[serde(default)]
    pub backend: Vec<Backend>,
}

/// One compute backend, such as CUDA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backend {
    /// Unique id. The search filter uses it.
    pub id: Slug,
    /// Display name.
    pub name: String,
    /// Crates whose presence as a dependency, or as a listed crate, shows this backend.
    #[serde(default)]
    pub dependencies: Vec<CrateName>,
    /// Feature names that show this backend. Lowercase.
    #[serde(default)]
    pub features: Vec<String>,
}

impl Backend {
    /// Lists every rule this row breaks on its own, without other rows.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.trim().is_empty() {
            out.push("name must not be empty".to_owned());
        }
        if self.name.contains(RUNS_ON_SEPARATOR.trim_end()) {
            out.push(format!(
                "name `{}` must not hold a comma: the `runs_on` fact joins names with commas",
                self.name
            ));
        }
        if self.name == NO_GPU_FEATURE || self.name == NOT_MEASURED {
            out.push(format!(
                "name `{}` is reserved: the `runs_on` fact uses it for a project with no backend row",
                self.name
            ));
        }
        if self.id.as_str() == NO_GPU_FEATURE_ID || self.id.as_str() == NOT_MEASURED_ID {
            out.push(format!(
                "id `{}` is reserved: the search filter uses it for a project with no backend row",
                self.id
            ));
        }
        if self.dependencies.is_empty() && self.features.is_empty() {
            out.push("add a dependency or a feature: the row can show nothing".to_owned());
        }
        for feature in &self.features {
            if !is_feature_name(feature) {
                out.push(format!(
                    "feature `{feature}` must be lowercase ASCII letters, digits, `-`, `_` or `+`"
                ));
            }
        }
        out
    }
}

/// Whether `value` is a lowercase Cargo feature name.
fn is_feature_name(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'+')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "id = \"cuda\"\nname = \"CUDA\"\n";

    fn backend(text: &str) -> Backend {
        toml::from_str(text).expect("backend parses")
    }

    #[test]
    fn a_valid_row_passes() {
        let row = backend(&format!(
            "{HEAD}dependencies = [\"cudarc\"]\nfeatures = [\"cuda\"]\n"
        ));
        assert!(row.problems().is_empty());
        assert_eq!(row.dependencies[0].as_str(), "cudarc");
    }

    #[test]
    fn a_row_that_shows_nothing_is_reported() {
        assert_eq!(
            backend(HEAD).problems(),
            ["add a dependency or a feature: the row can show nothing"]
        );
    }

    #[test]
    fn a_name_with_a_comma_or_a_reserved_word_is_reported() {
        for (name, needle) in [
            ("Vulkan, Metal", "must not hold a comma"),
            (" ", "must not be empty"),
            ("GPU support unknown", "is reserved"),
            ("Not measured", "is reserved"),
        ] {
            let row = backend(&format!(
                "id = \"x\"\nname = \"{name}\"\nfeatures = [\"x\"]\n"
            ));
            let found = row.problems();
            assert!(
                found.iter().any(|problem| problem.contains(needle)),
                "{name}: {found:?}"
            );
        }
    }

    #[test]
    fn a_reserved_id_is_reported() {
        for id in [NO_GPU_FEATURE_ID, NOT_MEASURED_ID] {
            let row = backend(&format!(
                "id = \"{id}\"\nname = \"X\"\nfeatures = [\"x\"]\n"
            ));
            assert!(row.problems()[0].contains("is reserved"), "{id}");
        }
    }

    #[test]
    fn a_feature_that_is_not_lowercase_is_reported() {
        let row = backend(&format!("{HEAD}features = [\"CUDA\"]\n"));
        assert_eq!(
            row.problems(),
            ["feature `CUDA` must be lowercase ASCII letters, digits, `-`, `_` or `+`"]
        );
    }

    #[test]
    fn a_bad_crate_name_and_an_unknown_field_are_parse_errors() {
        assert!(toml::from_str::<Backend>(&format!("{HEAD}dependencies = [\"a b\"]\n")).is_err());
        assert!(toml::from_str::<Backend>(&format!("{HEAD}extra = 1\n")).is_err());
    }

    #[test]
    fn an_empty_file_is_an_empty_table() {
        let table: Backends = toml::from_str("").expect("parses");
        assert!(table.backend.is_empty() && table.python.is_empty() && table.not_native.is_empty());
        assert!(table.native_build.is_empty());
    }
}
