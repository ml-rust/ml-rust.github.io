use std::collections::{BTreeMap, BTreeSet};

use super::diagnostics::Diagnostics;
use super::parse::Parsed;
use crate::entry::Entry;
use crate::ids::Slug;
use crate::registry::Showcase;

/// Checks that each slug of `showcase.toml` is unique and names an org-affiliated entry.
pub(crate) fn check_showcase(
    file: &Parsed<Showcase>,
    entries: &BTreeMap<Slug, Entry>,
    diags: &mut Diagnostics,
) {
    let mut seen = BTreeSet::new();
    for slug in &file.value.project {
        if !seen.insert(slug) {
            diags.push(
                &file.path,
                format!("project `{slug}` is listed twice. Remove one"),
            );
        }
        match entries.get(slug) {
            None => diags.push(
                &file.path,
                format!("project `{slug}` names no entry. Add the entry or remove the slug"),
            ),
            Some(entry) if !entry.org_affiliated => diags.push(
                &file.path,
                format!(
                    "project `{slug}` is not `org_affiliated`. List only ML Rust projects here"
                ),
            ),
            Some(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::catalogue::Catalogue;
    use crate::testing::{errors, put, valid};

    #[test]
    fn a_missing_file_reads_as_an_empty_list() {
        let catalogue = Catalogue::load(valid()).expect("valid data");
        assert!(catalogue.showcase().is_empty());
    }

    #[test]
    fn the_list_keeps_file_order() {
        let mut src = valid();
        put(&mut src, "showcase.toml", "project = [\"beta\"]\n");
        let catalogue = Catalogue::load(src).expect("valid data");
        let slugs: Vec<&str> = catalogue.showcase().iter().map(|s| s.as_str()).collect();
        assert_eq!(slugs, ["beta"]);
    }

    #[test]
    fn each_rule_break_names_the_file_and_the_slug() {
        let mut src = valid();
        put(
            &mut src,
            "showcase.toml",
            "project = [\"beta\", \"beta\", \"missing\", \"alpha\"]\n",
        );
        let found = errors(src);
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found[0].contains("`beta` is listed twice"), "{found:?}");
        assert!(found[1].contains("`missing` names no entry"), "{found:?}");
        assert!(found[2].contains("`alpha` is not"), "{found:?}");
        assert!(found.iter().all(|e| e.starts_with("showcase.toml: ")));
    }
}
