use std::collections::{BTreeMap, BTreeSet};

use super::diagnostics::Diagnostics;
use super::parse::Parsed;
use crate::ids::{CrateName, Slug};
use crate::registry::Backends;

/// Checks the backend table. Each name maps to one row and appears once.
pub(crate) fn check_backends(file: &Parsed<Backends>, diags: &mut Diagnostics) {
    let table = &file.value;
    let mut ids: BTreeSet<&Slug> = BTreeSet::new();
    let mut dependencies: BTreeMap<String, &Slug> = BTreeMap::new();
    let mut features: BTreeMap<&str, &Slug> = BTreeMap::new();
    for backend in &table.backend {
        for problem in backend.problems() {
            diags.push(&file.path, format!("backend `{}`: {problem}", backend.id));
        }
        if !ids.insert(&backend.id) {
            diags.push(
                &file.path,
                format!(
                    "backend `{}` is defined twice. Keep one table with this id",
                    backend.id
                ),
            );
        }
        for name in &backend.dependencies {
            let key = name.normalized();
            if let Some(owner) = dependencies.insert(key, &backend.id) {
                diags.push(
                    &file.path,
                    format!(
                        "dependency `{name}` maps to backends `{owner}` and `{}`. Keep it in one row",
                        backend.id
                    ),
                );
            }
        }
        for feature in &backend.features {
            if let Some(owner) = features.insert(feature, &backend.id) {
                diags.push(
                    &file.path,
                    format!(
                        "feature `{feature}` maps to backends `{owner}` and `{}`. Keep it in one row",
                        backend.id
                    ),
                );
            }
        }
    }
    check_list("python", &table.python, file, diags);
    check_list("not_native", &table.not_native, file, diags);
}

/// Reports a crate that `list` names twice.
fn check_list(list: &str, names: &[CrateName], file: &Parsed<Backends>, diags: &mut Diagnostics) {
    let mut seen = BTreeSet::new();
    for name in names {
        if !seen.insert(name.normalized()) {
            diags.push(
                &file.path,
                format!("`{list}` lists crate `{name}` twice. Keep one"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::catalogue::Catalogue;
    use crate::testing::{errors, expect_error, put, valid};

    const CUDA: &str = "[[backend]]\nid = \"cuda\"\nname = \"CUDA\"\ndependencies = [\"cudarc\"]\nfeatures = [\"cuda\"]\n";

    fn with_table(text: &str) -> Vec<String> {
        let mut src = valid();
        put(&mut src, "backends.toml", text);
        errors(src)
    }

    #[test]
    fn a_valid_table_loads_and_keeps_its_order() {
        let mut src = valid();
        let metal = "[[backend]]\nid = \"metal\"\nname = \"Metal\"\nfeatures = [\"metal\"]\n";
        put(
            &mut src,
            "backends.toml",
            &format!("python = [\"pyo3\"]\nnot_native = [\"js-sys\"]\n{CUDA}{metal}"),
        );
        let catalogue = Catalogue::load(src).expect("valid data");
        let names: Vec<&str> = catalogue
            .backends()
            .backend
            .iter()
            .map(|row| row.name.as_str())
            .collect();
        assert_eq!(names, ["CUDA", "Metal"]);
        assert_eq!(catalogue.backends().python[0].as_str(), "pyo3");
    }

    #[test]
    fn an_absent_file_reads_as_an_empty_table() {
        let mut src = valid();
        src.backends = None;
        let catalogue = Catalogue::load(src).expect("valid data");
        assert!(catalogue.backends().backend.is_empty());
        assert!(catalogue.backends().python.is_empty());
    }

    #[test]
    fn a_row_rule_names_the_backend() {
        let mut src = valid();
        put(
            &mut src,
            "backends.toml",
            "[[backend]]\nid = \"cuda\"\nname = \"CUDA\"\n",
        );
        expect_error(src, "backend `cuda`: add a dependency or a feature");
    }

    #[test]
    fn ids_must_be_unique() {
        let found = with_table(&format!("{CUDA}{CUDA}"));
        assert!(
            found
                .iter()
                .any(|message| message.contains("backend `cuda` is defined twice")),
            "{found:?}"
        );
    }

    #[test]
    fn a_name_in_two_rows_is_reported_in_any_spelling() {
        let other = "[[backend]]\nid = \"other\"\nname = \"Other\"\ndependencies = [\"CudaRC\"]\nfeatures = [\"cuda\"]\n";
        let found = with_table(&format!("{CUDA}{other}"));
        for needle in [
            "dependency `CudaRC` maps to backends `cuda` and `other`",
            "feature `cuda` maps to backends `cuda` and `other`",
        ] {
            assert!(
                found.iter().any(|message| message.contains(needle)),
                "{needle}: {found:?}"
            );
        }
    }

    #[test]
    fn a_repeat_in_a_list_is_reported() {
        let found =
            with_table("python = [\"pyo3\", \"pyo3\"]\nnot_native = [\"a-sys\", \"A_sys\"]\n");
        for needle in [
            "`python` lists crate `pyo3` twice",
            "`not_native` lists crate `A_sys` twice",
        ] {
            assert!(
                found.iter().any(|message| message.contains(needle)),
                "{needle}: {found:?}"
            );
        }
    }

    #[test]
    fn broken_toml_is_a_parse_error() {
        assert!(!with_table("[[backend]]\nid = \"Not A Slug\"\nname = \"X\"\n").is_empty());
    }
}
