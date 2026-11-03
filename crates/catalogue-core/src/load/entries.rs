use std::collections::{BTreeMap, BTreeSet};

use super::diagnostics::Diagnostics;
use super::parse::Parsed;
use crate::entry::Entry;
use crate::ids::{CrateName, Slug};

/// Checks entries against each other and against the topic list.
///
/// Returns the entries that have a valid slug, keyed by slug.
pub(crate) fn check_entries(
    files: Vec<Parsed<Entry>>,
    topic_ids: &BTreeSet<Slug>,
    diags: &mut Diagnostics,
) -> BTreeMap<Slug, Entry> {
    let mut entries: BTreeMap<Slug, Entry> = BTreeMap::new();
    let mut crate_owners: BTreeMap<CrateName, String> = BTreeMap::new();
    for Parsed { path, stem, value } in files {
        for problem in value.problems() {
            diags.push(&path, problem);
        }
        for topic in &value.topics {
            if !topic_ids.contains(topic) {
                diags.push(&path, format!("topic `{topic}` does not exist"));
            }
        }
        for name in &value.crates {
            match crate_owners.get(name) {
                Some(owner) => {
                    diags.push(
                        &path,
                        format!("crate `{name}` is already listed in {owner}"),
                    );
                }
                None => {
                    crate_owners.insert(name.clone(), path.clone());
                }
            }
        }
        match Slug::new(stem) {
            Ok(slug) if entries.contains_key(&slug) => {
                diags.push(&path, format!("entry `{slug}` is defined twice"));
            }
            Ok(slug) => {
                entries.insert(slug, value);
            }
            Err(error) => diags.push(&path, format!("file name is not a slug: {error}")),
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use crate::testing::{BETA, errors, expect_error, put, valid};

    #[test]
    fn valid_data_passes() {
        assert!(errors(valid()).is_empty());
    }

    #[test]
    fn file_stem_must_be_a_slug() {
        let mut src = valid();
        put(&mut src, "entries/Bad_Name.toml", BETA);
        expect_error(src, "file name is not a slug");
    }

    #[test]
    fn slug_must_be_unique() {
        let mut src = valid();
        put(
            &mut src,
            "other/entries/beta.toml",
            &BETA.replace("beta-crate", "other-crate"),
        );
        expect_error(src, "defined twice");
    }

    #[test]
    fn entry_topics_must_exist() {
        let mut src = valid();
        put(
            &mut src,
            "entries/beta.toml",
            &BETA.replace("topics = [\"tensors\"]", "topics = [\"nope\"]"),
        );
        expect_error(src, "topic `nope` does not exist");
    }

    #[test]
    fn crate_names_must_be_unique() {
        let mut src = valid();
        put(
            &mut src,
            "entries/gamma.toml",
            &BETA.replace("crates = []", "crates = [\"alpha-crate\"]"),
        );
        expect_error(src, "crate `alpha-crate` is already listed");
    }

    #[test]
    fn per_file_rules_are_reported() {
        let mut src = valid();
        put(
            &mut src,
            "entries/beta.toml",
            &BETA.replace("name = \"Beta\"", "name = \"\""),
        );
        expect_error(src, "name must not be empty");
    }
}
