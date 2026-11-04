use std::collections::{BTreeMap, BTreeSet};

use super::diagnostics::Diagnostics;
use super::parse::Parsed;
use crate::entry::Entry;
use crate::ids::{CrateName, Slug, normalize_repo_url};

/// Checks entries against each other and against the topic and category lists.
///
/// Returns the entries that have a valid slug, keyed by slug.
pub(crate) fn check_entries(
    files: Vec<Parsed<Entry>>,
    topic_ids: &BTreeSet<Slug>,
    category_ids: &BTreeSet<Slug>,
    diags: &mut Diagnostics,
) -> BTreeMap<Slug, Entry> {
    let mut entries: BTreeMap<Slug, Entry> = BTreeMap::new();
    let mut crate_owners: BTreeMap<CrateName, String> = BTreeMap::new();
    let mut repo_owners: BTreeMap<String, String> = BTreeMap::new();
    let mut paths: BTreeMap<Slug, String> = BTreeMap::new();
    for Parsed { path, stem, value } in files {
        for problem in value.problems() {
            diags.push(&path, problem);
        }
        for topic in &value.topics {
            if !topic_ids.contains(topic) {
                diags.push(&path, format!("topic `{topic}` does not exist"));
            }
        }
        if let Some(category) = value
            .category
            .as_ref()
            .filter(|category| !category_ids.contains(category))
        {
            diags.push(
                &path,
                format!(
                    "category `{category}` does not exist. Use an id from `categories.toml`, or remove the field"
                ),
            );
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
        let slug = Slug::new(stem);
        if let Some(repo) = normalize_repo_url(value.repo.as_str()) {
            match (repo_owners.get(&repo), &slug) {
                (Some(first), Ok(slug)) => diags.push(
                    &path,
                    format!("entry `{slug}` has the same repository `{repo}` as entry `{first}`"),
                ),
                (Some(first), Err(_)) => diags.push(
                    &path,
                    format!("repository `{repo}` is already used by entry `{first}`"),
                ),
                (None, Ok(slug)) => {
                    repo_owners.insert(repo, slug.to_string());
                }
                (None, Err(_)) => {}
            }
        }
        match slug {
            Ok(slug) if entries.contains_key(&slug) => {
                diags.push(&path, format!("entry `{slug}` is defined twice"));
            }
            Ok(slug) => {
                paths.insert(slug.clone(), path);
                entries.insert(slug, value);
            }
            Err(error) => diags.push(&path, format!("file name is not a slug: {error}")),
        }
    }
    check_part_of(&entries, &paths, diags);
    entries
}

/// Checks each `part_of`: it names another entry, and that entry names no parent of its own.
fn check_part_of(
    entries: &BTreeMap<Slug, Entry>,
    paths: &BTreeMap<Slug, String>,
    diags: &mut Diagnostics,
) {
    for (slug, entry) in entries {
        let Some(parent) = &entry.part_of else {
            continue;
        };
        let path = paths.get(slug).map_or(slug.as_str(), String::as_str);
        if parent == slug {
            diags.push(
                path,
                format!(
                    "part_of `{parent}` names the entry itself. Name another entry, or remove the field"
                ),
            );
            continue;
        }
        match entries.get(parent).map(|found| found.part_of.as_ref()) {
            None => diags.push(
                path,
                format!(
                    "part_of `{parent}` does not exist. Use the slug of an entry, or remove the field"
                ),
            ),
            Some(Some(grandparent)) => diags.push(
                path,
                format!(
                    "part_of `{parent}` names an entry that is part of `{grandparent}`. Name `{grandparent}`, or remove the field"
                ),
            ),
            Some(None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{ALPHA, BETA, errors, expect_error, put, valid};

    #[test]
    fn valid_data_passes() {
        assert!(errors(valid()).is_empty());
    }

    #[test]
    fn part_of_can_name_another_entry() {
        let mut src = valid();
        put(
            &mut src,
            "entries/beta.toml",
            &format!("part_of = \"alpha\"\n{BETA}"),
        );
        assert!(errors(src).is_empty());
    }

    #[test]
    fn part_of_must_name_an_entry_that_exists() {
        let mut src = valid();
        put(
            &mut src,
            "entries/beta.toml",
            &format!("part_of = \"ghost\"\n{BETA}"),
        );
        expect_error(
            src,
            "entries/beta.toml: part_of `ghost` does not exist. Use the slug of an entry, or remove the field",
        );
    }

    #[test]
    fn part_of_cannot_name_the_entry_itself() {
        let mut src = valid();
        put(
            &mut src,
            "entries/beta.toml",
            &format!("part_of = \"beta\"\n{BETA}"),
        );
        expect_error(
            src,
            "entries/beta.toml: part_of `beta` names the entry itself. Name another entry, or remove the field",
        );
    }

    #[test]
    fn a_parent_cannot_have_a_parent() {
        let mut src = valid();
        put(
            &mut src,
            "entries/alpha.toml",
            &format!("part_of = \"beta\"\n{ALPHA}"),
        );
        put(
            &mut src,
            "entries/gamma.toml",
            &format!(
                "part_of = \"alpha\"\n{}",
                BETA.replace("ml-rust/beta\"", "ml-rust/gamma\"")
            ),
        );
        expect_error(
            src,
            "entries/gamma.toml: part_of `alpha` names an entry that is part of `beta`. Name `beta`, or remove the field",
        );
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
    fn repositories_must_be_unique() {
        let mut src = valid();
        put(
            &mut src,
            "entries/gamma.toml",
            &BETA.replace("name = \"Beta\"", "name = \"Gamma\""),
        );
        expect_error(
            src,
            "entry `gamma` has the same repository `github.com/ml-rust/beta` as entry `beta`",
        );
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
