use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::ids::CrateName;

/// The longest keyword crates.io stores.
const KEYWORD_MAX_CHARS: usize = 50;

/// Contents of `data/discovery.toml`.
///
/// The rules that select crates.io crates as candidates for review.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discovery {
    /// crates.io category slugs. A slug also matches its sub-categories.
    #[serde(default)]
    pub categories: Vec<String>,
    /// crates.io keywords, lowercase and trimmed.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Crates whose dependents become candidates.
    #[serde(default)]
    pub core_crates: Vec<CrateName>,
}

impl Discovery {
    /// Lists every rule this file breaks.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.categories.is_empty() && self.keywords.is_empty() && self.core_crates.is_empty() {
            out.push("categories, keywords and core_crates must not all be empty".to_owned());
        }
        for category in &self.categories {
            if !is_category_slug(category) {
                out.push(format!(
                    "category `{category}` must match `[a-z0-9_-]+(::[a-z0-9_-]+)*`"
                ));
            }
        }
        for keyword in &self.keywords {
            if let Some(problem) = keyword_problem(keyword) {
                out.push(problem);
            }
        }
        duplicates(
            "category",
            self.categories.iter().map(|c| (c.clone(), c.as_str())),
            &mut out,
        );
        duplicates(
            "keyword",
            self.keywords.iter().map(|k| (k.clone(), k.as_str())),
            &mut out,
        );
        duplicates(
            "core crate",
            self.core_crates
                .iter()
                .map(|c| (c.normalized(), c.as_str())),
            &mut out,
        );
        out
    }
}

/// Whether `value` is `segment(::segment)*` with segments of `[a-z0-9_-]`.
fn is_category_slug(value: &str) -> bool {
    value.split("::").all(|segment| {
        !segment.is_empty()
            && segment
                .bytes()
                .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
    })
}

fn keyword_problem(keyword: &str) -> Option<String> {
    if keyword.trim().is_empty() {
        Some("a keyword must not be empty".to_owned())
    } else if keyword != keyword.trim() {
        Some(format!(
            "keyword `{keyword}` must not have surrounding whitespace"
        ))
    } else if *keyword != keyword.to_lowercase() {
        Some(format!("keyword `{keyword}` must be lowercase"))
    } else if keyword.chars().count() > KEYWORD_MAX_CHARS {
        Some(format!(
            "keyword `{keyword}` must be at most {KEYWORD_MAX_CHARS} characters"
        ))
    } else {
        None
    }
}

/// Reports each item whose key repeats an earlier key. Items are `(key, shown text)`.
fn duplicates<'a>(
    what: &str,
    items: impl Iterator<Item = (String, &'a str)>,
    out: &mut Vec<String>,
) {
    let mut seen = BTreeSet::new();
    for (key, shown) in items {
        if !seen.insert(key) {
            out.push(format!("{what} `{shown}` is listed twice"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Discovery {
        toml::from_str(text).expect("parses")
    }

    fn found(text: &str, needle: &str) -> bool {
        parse(text).problems().iter().any(|p| p.contains(needle))
    }

    #[test]
    fn valid_file_passes() {
        let file = parse(
            "categories = [\"science::ml\", \"development-tools::cargo-plugins\"]\n\
             keywords = [\"llm\", \"deep-learning\"]\n\
             core_crates = [\"burn\", \"candle-core\"]\n",
        );
        assert!(file.problems().is_empty());
    }

    #[test]
    fn one_non_empty_list_is_enough() {
        assert!(parse("keywords = [\"llm\"]").problems().is_empty());
        assert!(parse("categories = [\"science\"]").problems().is_empty());
        assert!(parse("core_crates = [\"burn\"]").problems().is_empty());
    }

    #[test]
    fn all_lists_empty_is_reported() {
        assert!(found("", "must not all be empty"));
        assert!(found(
            "categories = []\nkeywords = []\ncore_crates = []\n",
            "must not all be empty"
        ));
    }

    #[test]
    fn bad_category_slugs_are_reported() {
        for bad in ["Science", "science::", "::ml", "a:b", "a b", "a:::b", ""] {
            let text = format!("categories = [\"{bad}\"]\n");
            assert!(found(&text, "must match"), "{bad}");
        }
    }

    #[test]
    fn keyword_rules_are_reported() {
        assert!(found("keywords = [\"\"]", "must not be empty"));
        assert!(found("keywords = [\"  \"]", "must not be empty"));
        assert!(found("keywords = [\" llm\"]", "surrounding whitespace"));
        assert!(found("keywords = [\"LLM\"]", "must be lowercase"));
        let long = "a".repeat(51);
        assert!(found(&format!("keywords = [\"{long}\"]"), "at most 50"));
        let edge = "a".repeat(50);
        assert!(
            parse(&format!("keywords = [\"{edge}\"]"))
                .problems()
                .is_empty()
        );
    }

    #[test]
    fn duplicates_are_reported_per_list() {
        assert!(found(
            "categories = [\"science::ml\", \"science::ml\"]",
            "category `science::ml` is listed twice"
        ));
        assert!(found(
            "keywords = [\"llm\", \"llm\"]",
            "keyword `llm` is listed twice"
        ));
    }

    #[test]
    fn crate_names_compare_the_crates_io_way() {
        assert!(found(
            "core_crates = [\"candle-core\", \"Candle_Core\"]",
            "core crate `Candle_Core` is listed twice"
        ));
    }

    #[test]
    fn the_same_text_in_different_lists_is_not_a_duplicate() {
        let file = parse("categories = [\"ml\"]\nkeywords = [\"ml\"]\n");
        assert!(file.problems().is_empty());
    }

    #[test]
    fn invalid_crate_name_and_unknown_field_fail_to_parse() {
        assert!(toml::from_str::<Discovery>("core_crates = [\"1bad\"]").is_err());
        assert!(toml::from_str::<Discovery>("extra = 1").is_err());
    }
}
