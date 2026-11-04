use serde::{Deserialize, Serialize};

use super::mappings::mapping_problems;
use crate::ids::Slug;

/// Contents of `data/categories.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Categories {
    /// The `[[category]]` tables. The order decides which of several matches `catalogue seed` picks.
    #[serde(default)]
    pub category: Vec<Category>,
}

/// What a project is, such as a vector database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    /// Unique id.
    pub id: Slug,
    /// Display name.
    pub name: String,
    /// One plain sentence that defines the category. It holds no claim that needs evidence.
    pub blurb: String,
    /// crates.io keywords that map a crate to this category. Lowercase and trimmed.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// crates.io category slugs that map a crate to this category. A slug also matches its sub-categories.
    #[serde(default)]
    pub categories: Vec<String>,
    /// Whole words or phrases that map a crate to this category. A term match wins over a keyword or slug match.
    ///
    /// They read the same texts as the terms of a topic. Lowercase words of `[a-z0-9]`, separated by one space.
    #[serde(default)]
    pub terms: Vec<String>,
}

impl Category {
    /// Lists every rule this category breaks on its own, without other categories.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.trim().is_empty() {
            out.push("name must not be empty".to_owned());
        }
        if self.blurb.trim().is_empty() {
            out.push("blurb must not be empty".to_owned());
        }
        out.extend(mapping_problems(
            &self.keywords,
            &self.categories,
            &self.terms,
        ));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "id = \"tools\"\nname = \"Tools\"\nblurb = \"Tools for people.\"\n";

    fn category(text: &str) -> Category {
        toml::from_str(text).expect("category parses")
    }

    #[test]
    fn a_valid_category_passes_and_its_mappings_default_to_empty() {
        let found = category(HEAD);
        assert!(found.problems().is_empty());
        assert!(found.keywords.is_empty() && found.categories.is_empty() && found.terms.is_empty());
    }

    #[test]
    fn an_empty_name_or_blurb_is_reported() {
        let found = category("id = \"tools\"\nname = \" \"\nblurb = \"\"\n").problems();
        assert_eq!(found, ["name must not be empty", "blurb must not be empty"]);
    }

    #[test]
    fn a_missing_blurb_is_a_parse_error() {
        assert!(toml::from_str::<Category>("id = \"tools\"\nname = \"Tools\"\n").is_err());
    }

    #[test]
    fn bad_mappings_break_the_same_rules_as_a_topic() {
        for (field, value, needle) in [
            ("keywords", "\"LLM\"", "must be lowercase"),
            ("categories", "\"Science::ML\"", "must match"),
            ("terms", "\"two  spaces\"", "must be lowercase words"),
        ] {
            let found = category(&format!("{HEAD}{field} = [{value}]\n")).problems();
            assert!(
                found.iter().any(|problem| problem.contains(needle)),
                "{field} {value}: {found:?}"
            );
        }
    }

    #[test]
    fn an_unknown_field_is_rejected() {
        assert!(toml::from_str::<Category>(&format!("{HEAD}extra = 1\n")).is_err());
    }
}
