use std::collections::btree_map::Entry as Slot;
use std::collections::{BTreeMap, BTreeSet};

use super::super::diagnostics::Diagnostics;
use super::super::parse::Parsed;
use super::supersedes::Link;
use super::{approval, references, supersedes};
use crate::entry::Entry;
use crate::ids::Slug;
use crate::judgment::Judgment;

/// Data that judgments point at.
pub(crate) struct Refs<'a> {
    pub entries: &'a BTreeMap<Slug, Entry>,
    pub topics: &'a BTreeSet<Slug>,
    /// Lowercase maintainer login mapped to `org_member`.
    pub maintainers: &'a BTreeMap<String, bool>,
}

/// The issue number a judgment file name stands for.
fn issue_number(stem: &str) -> Option<u64> {
    stem.parse::<u64>()
        .ok()
        .filter(|number| *number > 0 && number.to_string() == stem)
}

/// Checks judgments on their own and against the rest of the data.
///
/// Returns the judgments that have a valid, unused issue number.
pub(crate) fn check_judgments(
    files: Vec<Parsed<Judgment>>,
    refs: &Refs<'_>,
    diags: &mut Diagnostics,
) -> BTreeMap<u64, Judgment> {
    let mut accepted: BTreeMap<u64, Judgment> = BTreeMap::new();
    let mut links: Vec<Link> = Vec::new();
    for Parsed { path, stem, value } in files {
        let number = issue_number(&stem);
        match number {
            None => diags.push(
                &path,
                format!("file name `{stem}` must be a positive issue number"),
            ),
            Some(number) if !value.issue_matches(number) => diags.push(
                &path,
                format!(
                    "issue must be {}{number} to match the file name",
                    crate::judgment::ISSUE_URL_PREFIX
                ),
            ),
            Some(_) => {}
        }
        for problem in value.problems() {
            diags.push(&path, problem);
        }
        references::check(&path, &value, refs, diags);
        approval::check(&path, &value, refs, diags);
        links.extend(Link::of(&path, &value));
        if let Some(number) = number {
            match accepted.entry(number) {
                Slot::Vacant(slot) => {
                    slot.insert(value);
                }
                Slot::Occupied(_) => {
                    diags.push(&path, format!("issue number {number} is used twice"));
                }
            }
        }
    }
    supersedes::check(&links, diags);
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{JUDGMENT, errors, expect_error, put, valid};

    #[test]
    fn issue_numbers_are_canonical_positive_integers() {
        assert_eq!(issue_number("7"), Some(7));
        assert_eq!(issue_number("120"), Some(120));
        for bad in ["0", "07", "-1", "abc", "", "1.5", "+7"] {
            assert_eq!(issue_number(bad), None, "{bad}");
        }
    }

    #[test]
    fn valid_judgment_passes() {
        assert!(errors(valid()).is_empty());
    }

    #[test]
    fn file_name_must_be_a_number() {
        let mut src = valid();
        put(&mut src, "judgments/seven.toml", JUDGMENT);
        expect_error(src, "must be a positive issue number");
    }

    #[test]
    fn issue_link_must_match_the_file_name() {
        let mut src = valid();
        put(&mut src, "judgments/8.toml", JUDGMENT);
        expect_error(src, "to match the file name");
    }

    #[test]
    fn issue_numbers_must_be_unique() {
        let mut src = valid();
        put(&mut src, "other/judgments/7.toml", JUDGMENT);
        expect_error(src, "issue number 7 is used twice");
    }

    #[test]
    fn per_file_rules_are_reported() {
        let mut src = valid();
        put(
            &mut src,
            "judgments/7.toml",
            &JUDGMENT.replace(
                "evidence = [\"https://example.com/proof\"]",
                "evidence = []",
            ),
        );
        expect_error(src, "evidence needs at least one link");
    }
}
