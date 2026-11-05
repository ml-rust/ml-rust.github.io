use std::collections::BTreeSet;

use super::diagnostics::Diagnostics;
use super::mappings::{Mapped, MappingNames, check_mappings};
use super::parse::Parsed;
use crate::ids::Slug;
use crate::registry::Categories;

/// What the mapping diagnostics of `categories.toml` call things.
///
/// A crates.io slug keeps its site's name, so it never reads as a catalogue category.
const NAMES: MappingNames = MappingNames {
    owner: "category",
    owners: "categories",
    slug: "crates.io category",
};

/// Checks the category list and returns the set of category ids.
pub(crate) fn check_categories(
    file: &Parsed<Categories>,
    diags: &mut Diagnostics,
) -> BTreeSet<Slug> {
    let categories = &file.value.category;
    let mut ids: BTreeSet<Slug> = BTreeSet::new();
    for category in categories {
        for problem in category.problems() {
            diags.push(&file.path, format!("category `{}`: {problem}", category.id));
        }
        if !ids.insert(category.id.clone()) {
            diags.push(
                &file.path,
                format!(
                    "category `{}` is defined twice. Keep one table with this id",
                    category.id
                ),
            );
        }
    }
    check_mappings(
        NAMES,
        categories.iter().map(|category| Mapped {
            id: &category.id,
            keywords: &category.keywords,
            categories: &category.categories,
            terms: &category.terms,
        }),
        &file.path,
        diags,
    );
    ids
}

#[cfg(test)]
mod tests {
    use crate::catalogue::Catalogue;
    use crate::testing::{ALPHA, errors, expect_error, put, valid};

    const TOOLS: &str = "[[category]]\nid = \"tools\"\nname = \"Tools\"\nblurb = \"Tools.\"\n";
    const MORE: &str = "[[category]]\nid = \"more\"\nname = \"More\"\nblurb = \"More.\"\n";

    fn with_categories(tools: &str, more: &str) -> Vec<String> {
        let mut src = valid();
        put(
            &mut src,
            "categories.toml",
            &format!("{TOOLS}{tools}{MORE}{more}"),
        );
        errors(src)
    }

    #[test]
    fn distinct_mappings_pass() {
        let found = with_categories(
            "keywords = [\"a\"]\ncategories = [\"x\"]\nterms = [\"p\"]\n",
            "keywords = [\"b\"]\ncategories = [\"x::y\"]\nterms = [\"q\"]\n",
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_term_keyword_and_slug_map_to_one_category() {
        let tools = "keywords = [\"k\"]\ncategories = [\"s\"]\nterms = [\"vector database\"]\n";
        let found = with_categories(tools, tools);
        for needle in [
            "term `vector database` maps to categories `tools` and `more`",
            "keyword `k` maps to categories `tools` and `more`",
            "crates.io category `s` maps to categories `tools` and `more`",
        ] {
            assert!(
                found.iter().any(|message| message.contains(needle)),
                "{needle}: {found:?}"
            );
        }
    }

    #[test]
    fn a_term_a_topic_also_uses_is_allowed() {
        let mut src = valid();
        put(
            &mut src,
            "topics.toml",
            "[[topic]]\nid = \"tensors\"\nname = \"Tensors\"\nblurb = \"A job.\"\nterms = [\"vector database\"]\n",
        );
        put(
            &mut src,
            "categories.toml",
            &format!("{TOOLS}terms = [\"vector database\"]\n"),
        );
        assert!(errors(src).is_empty());
    }

    #[test]
    fn a_repeat_in_one_category_is_reported() {
        let found = with_categories("terms = [\"llm\", \"llm\"]\n", "");
        assert!(
            found
                .iter()
                .any(|message| message.contains("category `tools`: term `llm` is listed twice")),
            "{found:?}"
        );
    }

    #[test]
    fn ids_must_be_unique() {
        let mut src = valid();
        put(&mut src, "categories.toml", &format!("{TOOLS}{TOOLS}"));
        expect_error(src, "category `tools` is defined twice");
    }

    #[test]
    fn per_category_rules_name_the_category() {
        let mut src = valid();
        put(
            &mut src,
            "categories.toml",
            "[[category]]\nid = \"tools\"\nname = \"Tools\"\nblurb = \"\"\n",
        );
        expect_error(src, "category `tools`: blurb must not be empty");
    }

    #[test]
    fn an_unknown_id_on_an_entry_is_reported_with_the_next_action() {
        let mut src = valid();
        put(
            &mut src,
            "entries/alpha.toml",
            &format!("category = \"ghost\"\n{ALPHA}"),
        );
        expect_error(
            src,
            "entries/alpha.toml: category `ghost` does not exist. Use an id from `categories.toml`",
        );
    }

    #[test]
    fn a_category_with_no_entries_passes() {
        let mut src = valid();
        put(&mut src, "categories.toml", TOOLS);
        assert!(errors(src).is_empty());
    }

    #[test]
    fn an_absent_file_reads_as_no_categories_like_topics() {
        let mut src = valid();
        src.categories = None;
        let catalogue = Catalogue::load(src).expect("valid data");
        assert!(catalogue.categories().is_empty());
        let mut named = valid();
        named.categories = None;
        put(
            &mut named,
            "entries/alpha.toml",
            &format!("category = \"tools\"\n{ALPHA}"),
        );
        expect_error(named, "category `tools` does not exist");
    }

    #[test]
    fn two_categories_on_one_entry_are_a_parse_error() {
        let mut src = valid();
        put(
            &mut src,
            "entries/alpha.toml",
            &format!("category = [\"tools\", \"more\"]\n{ALPHA}"),
        );
        assert!(!errors(src).is_empty());
    }
}
