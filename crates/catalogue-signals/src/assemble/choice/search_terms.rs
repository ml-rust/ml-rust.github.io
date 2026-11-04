//! The terms the search index reads for one entry, as crates.io and GitHub state them.

use std::collections::BTreeSet;

use catalogue_core::Entry;

use crate::assemble::index::CrateIndex;
use crate::dump::DumpSnapshot;
use crate::history::RepoValues;

/// The keywords and category slugs of every listed crate of `entry`, and the repository topics of
/// `facts`, deduplicated, lowercase and sorted.
///
/// A crate the dump does not hold adds nothing. `None` for `facts`, as for a repo not on GitHub,
/// adds no topic. The terms are search input: no tier rule reads them.
pub(super) fn search_terms(
    entry: &Entry,
    dump: &DumpSnapshot,
    index: &CrateIndex,
    facts: Option<&RepoValues>,
) -> Vec<String> {
    let of_crates = entry
        .crates
        .iter()
        .filter_map(|name| index.id(name.as_str()))
        .flat_map(|id| [dump.keywords.get(&id), dump.categories.get(&id)])
        .flatten()
        .flatten();
    let topics = facts.map_or(&[][..], RepoValues::topics);
    let terms: BTreeSet<String> = of_crates
        .chain(topics)
        .map(|term| term.trim().to_lowercase())
        .filter(|term| !term.is_empty())
        .collect();
    terms.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::fixtures::{Dump, found_facts, library};
    use crate::github::{Facts, RepoFacts};

    fn dump() -> Dump {
        let mut dump = Dump::new();
        dump.add_crate(1, "tool");
        dump.add_crate(2, "tool-core");
        dump.add_crate(3, "other");
        dump
    }

    fn terms(dump: &Dump, crates: &[&str], facts: Option<&RepoValues>) -> Vec<String> {
        let index = CrateIndex::new(&dump.snapshot);
        search_terms(&library(crates), &dump.snapshot, &index, facts)
    }

    fn keywords(dump: &mut Dump, id: u32, words: &[&str]) {
        let owned = words.iter().map(|word| (*word).to_owned()).collect();
        dump.snapshot.keywords.insert(id, owned);
    }

    fn repo_with(topics: &[&str]) -> RepoValues {
        RepoValues::from(&RepoFacts::Found(Facts {
            topics: topics.iter().map(|topic| (*topic).to_owned()).collect(),
            ..found_facts()
        }))
    }

    #[test]
    fn every_listed_crate_and_the_repo_join_deduplicated_lowercase_and_sorted() {
        let mut dump = dump();
        keywords(&mut dump, 1, &["Tensor", "linear-algebra"]);
        keywords(&mut dump, 2, &["tensor", " GPU "]);
        keywords(&mut dump, 3, &["unlisted"]);
        dump.snapshot
            .categories
            .insert(1, vec!["science::math".to_owned()]);
        let repo = repo_with(&["Rust", "linear-algebra", ""]);
        assert_eq!(
            terms(&dump, &["tool", "tool-core"], Some(&repo)),
            ["gpu", "linear-algebra", "rust", "science::math", "tensor"]
        );
    }

    #[test]
    fn a_missing_crate_and_a_missing_or_gone_repo_add_nothing() {
        let dump = dump();
        assert!(terms(&dump, &["absent"], None).is_empty());
        assert!(terms(&dump, &["tool"], Some(&RepoValues::Gone)).is_empty());
    }
}
