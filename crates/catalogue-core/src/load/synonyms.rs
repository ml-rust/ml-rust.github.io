use std::collections::BTreeMap;

use super::diagnostics::Diagnostics;
use super::parse::Parsed;
use crate::registry::Synonyms;

/// Checks the synonym groups.
pub(crate) fn check_synonyms(file: &Parsed<Synonyms>, diags: &mut Diagnostics) {
    let mut homes: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, group) in file.value.group.iter().enumerate() {
        let number = index + 1;
        for problem in group.problems() {
            diags.push(&file.path, format!("group {number}: {problem}"));
        }
        for term in &group.terms {
            match homes.get(term.as_str()) {
                Some(first) => diags.push(
                    &file.path,
                    format!("group {number}: term `{term}` already appears in group {first}"),
                ),
                None => {
                    homes.insert(term.as_str(), number);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{errors, expect_error, put, valid};

    fn with_groups(text: &str) -> Vec<String> {
        let mut src = valid();
        put(&mut src, "synonyms.toml", text);
        errors(src)
    }

    #[test]
    fn disjoint_groups_pass() {
        let text =
            "[[group]]\nterms = [\"gpu\", \"cuda\"]\n[[group]]\nterms = [\"cpu\", \"processor\"]\n";
        assert!(with_groups(text).is_empty());
    }

    #[test]
    fn a_term_in_two_groups_is_reported() {
        let mut src = valid();
        put(
            &mut src,
            "synonyms.toml",
            "[[group]]\nterms = [\"gpu\", \"cuda\"]\n[[group]]\nterms = [\"cuda\", \"nvidia\"]\n",
        );
        expect_error(src, "term `cuda` already appears in group 1");
    }

    #[test]
    fn a_repeated_term_in_one_group_is_reported() {
        let found = with_groups("[[group]]\nterms = [\"gpu\", \"gpu\"]\n");
        assert!(
            found.iter().any(|m| m.contains("already appears")),
            "{found:?}"
        );
    }

    #[test]
    fn per_group_rules_are_reported() {
        let found = with_groups("[[group]]\nterms = [\"gpu\"]\n");
        assert!(
            found.iter().any(|m| m.contains("at least two terms")),
            "{found:?}"
        );
    }
}
