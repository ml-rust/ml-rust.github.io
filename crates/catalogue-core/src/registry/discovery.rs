use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::ids::CrateName;

/// The longest keyword crates.io stores.
const KEYWORD_MAX_CHARS: usize = 50;

/// The longest GitHub topic.
const TOPIC_MAX_CHARS: usize = 50;

/// Contents of `data/discovery.toml`.
///
/// The rules that select crates.io crates and GitHub repos as candidates for review.
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
    /// GitHub topics searched together with `language:Rust`.
    #[serde(default)]
    pub github_topics: Vec<String>,
    /// The fewest stars a GitHub lead has. Zero keeps every repo.
    #[serde(default)]
    pub github_min_stars: u32,
}

impl Discovery {
    /// Lists every rule this file breaks.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.categories.is_empty()
            && self.keywords.is_empty()
            && self.core_crates.is_empty()
            && self.github_topics.is_empty()
        {
            out.push(
                "categories, keywords, core_crates and github_topics must not all be empty"
                    .to_owned(),
            );
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
        for topic in &self.github_topics {
            if !is_github_topic(topic) {
                out.push(format!(
                    "GitHub topic `{topic}` must match `[a-z0-9][a-z0-9-]{{0,{}}}`",
                    TOPIC_MAX_CHARS - 1
                ));
            }
        }
        duplicates(
            "GitHub topic",
            self.github_topics.iter().map(|t| (t.clone(), t.as_str())),
            &mut out,
        );
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

/// Whether `value` is a GitHub topic: 1 to 50 of `[a-z0-9-]`, starting with `[a-z0-9]`.
fn is_github_topic(value: &str) -> bool {
    let mut bytes = value.bytes();
    let starts = bytes
        .next()
        .is_some_and(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9'));
    starts
        && value.len() <= TOPIC_MAX_CHARS
        && bytes.all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
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
            "categories = []\nkeywords = []\ncore_crates = []\ngithub_topics = []\n",
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
    fn github_fields_default_to_empty_and_zero() {
        let file = parse("keywords = [\"llm\"]");
        assert!(file.github_topics.is_empty());
        assert_eq!(file.github_min_stars, 0);
    }

    #[test]
    fn github_topics_alone_are_enough() {
        let file = parse("github_topics = [\"llm\"]\ngithub_min_stars = 10\n");
        assert!(file.problems().is_empty());
        assert_eq!(file.github_min_stars, 10);
    }

    #[test]
    fn github_topic_syntax_is_checked() {
        for bad in ["", "-llm", "LLM", "a_b", "a b", "a.b", "é"] {
            let text = format!("github_topics = [\"{bad}\"]\n");
            assert!(found(&text, "GitHub topic"), "{bad}");
        }
        let long = "a".repeat(51);
        assert!(found(
            &format!("github_topics = [\"{long}\"]"),
            "must match"
        ));
        let edge = "a".repeat(50);
        assert!(
            parse(&format!("github_topics = [\"{edge}\", \"a-1\", \"1\"]"))
                .problems()
                .is_empty()
        );
    }

    #[test]
    fn duplicate_github_topics_are_reported() {
        assert!(found(
            "github_topics = [\"llm\", \"llm\"]",
            "GitHub topic `llm` is listed twice"
        ));
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
