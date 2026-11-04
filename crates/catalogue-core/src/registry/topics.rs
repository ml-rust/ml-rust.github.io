use serde::{Deserialize, Serialize};
use url::Url;

use super::discovery::{is_category_slug, keyword_problem};
use crate::ids::Slug;

/// Contents of `data/topics.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topics {
    /// The `[[topic]]` tables.
    #[serde(default)]
    pub topic: Vec<Topic>,
}

/// A subject that groups entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topic {
    /// Unique id.
    pub id: Slug,
    /// Display name.
    pub name: String,
    /// The broader topic, if any.
    pub parent: Option<Slug>,
    /// Short sourced overview of the topic.
    pub summary: Option<TopicSummary>,
    /// crates.io keywords that map a crate to this topic. Lowercase and trimmed.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// crates.io category slugs that map a crate to this topic. A slug also matches its sub-categories.
    #[serde(default)]
    pub categories: Vec<String>,
    /// Whole words or phrases that map a crate to this topic when no keyword or category does.
    ///
    /// `catalogue seed` looks for them in the name and description of the primary crate.
    /// Lowercase words of `[a-z0-9]`, separated by one space.
    #[serde(default)]
    pub terms: Vec<String>,
}

/// A sourced overview of a topic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopicSummary {
    /// The overview text.
    pub text: String,
    /// Links that support the text. At least one.
    pub evidence: Vec<Url>,
}

impl Topic {
    /// Lists every rule this topic breaks on its own, without other topics.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.trim().is_empty() {
            out.push("name must not be empty".to_owned());
        }
        if let Some(summary) = &self.summary {
            if summary.text.trim().is_empty() {
                out.push("summary text must not be empty".to_owned());
            }
            if summary.evidence.is_empty() {
                out.push("summary needs at least one evidence link".to_owned());
            }
            for link in summary
                .evidence
                .iter()
                .filter(|link| link.scheme() != "https")
            {
                out.push(format!("summary evidence link `{link}` must use https"));
            }
        }
        for keyword in &self.keywords {
            if let Some(problem) = keyword_problem(keyword) {
                out.push(problem);
            }
        }
        for category in &self.categories {
            if !is_category_slug(category) {
                out.push(format!(
                    "category `{category}` must match `[a-z0-9_-]+(::[a-z0-9_-]+)*`"
                ));
            }
        }
        for term in &self.terms {
            if !is_term(term) {
                out.push(format!(
                    "term `{term}` must be lowercase words of `[a-z0-9]` separated by one space"
                ));
            }
        }
        out
    }
}

/// Whether `value` is words of `[a-z0-9]` separated by one space.
fn is_term(value: &str) -> bool {
    value.split(' ').all(|word| {
        !word.is_empty() && word.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topic(text: &str) -> Topic {
        toml::from_str(text).expect("topic parses")
    }

    #[test]
    fn valid_topic_passes() {
        let t = topic(
            "id = \"ml\"\nname = \"Machine learning\"\n[summary]\ntext = \"Overview.\"\nevidence = [\"https://example.com/a\"]\n",
        );
        assert!(t.problems().is_empty());
        assert!(topic("id = \"ml\"\nname = \"ML\"\n").problems().is_empty());
    }

    #[test]
    fn empty_name_is_reported() {
        assert_eq!(topic("id = \"ml\"\nname = \"\"\n").problems().len(), 1);
    }

    #[test]
    fn summary_without_evidence_is_reported() {
        let t =
            topic("id = \"ml\"\nname = \"ML\"\n[summary]\ntext = \"Overview.\"\nevidence = []\n");
        assert!(t.problems().iter().any(|p| p.contains("evidence")));
    }

    #[test]
    fn summary_evidence_must_use_https() {
        let t = topic(
            "id = \"ml\"\nname = \"ML\"\n[summary]\ntext = \"Overview.\"\nevidence = [\"javascript:alert(1)\", \"http://example.com/a\"]\n",
        );
        let found = t.problems();
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found.iter().all(|p| p.contains("must use https")),
            "{found:?}"
        );
    }

    #[test]
    fn keywords_and_categories_default_to_empty() {
        let t = topic("id = \"ml\"\nname = \"ML\"\n");
        assert!(t.keywords.is_empty() && t.categories.is_empty());
        let t = topic(
            "id = \"ml\"\nname = \"ML\"\nkeywords = [\"llm\"]\ncategories = [\"science::ml\"]\n",
        );
        assert_eq!(t.keywords, ["llm"]);
        assert_eq!(t.categories, ["science::ml"]);
        assert!(t.problems().is_empty());
    }

    #[test]
    fn bad_keywords_and_categories_are_reported() {
        for (field, value, needle) in [
            ("keywords", "\"LLM\"", "must be lowercase"),
            ("keywords", "\" llm\"", "surrounding whitespace"),
            ("keywords", "\"\"", "must not be empty"),
            ("categories", "\"Science::ML\"", "must match"),
            ("categories", "\"science::\"", "must match"),
            ("categories", "\"\"", "must match"),
        ] {
            let t = topic(&format!(
                "id = \"ml\"\nname = \"ML\"\n{field} = [{value}]\n"
            ));
            assert!(
                t.problems().iter().any(|p| p.contains(needle)),
                "{field} {value}: {:?}",
                t.problems()
            );
        }
    }

    #[test]
    fn valid_terms_pass_and_default_to_empty() {
        assert!(topic("id = \"ml\"\nname = \"ML\"\n").terms.is_empty());
        let t = topic("id = \"ml\"\nname = \"ML\"\nterms = [\"vector database\", \"t sne\"]\n");
        assert_eq!(t.terms, ["vector database", "t sne"]);
        assert!(t.problems().is_empty());
    }

    #[test]
    fn bad_terms_are_reported() {
        for value in ["LLM", " llm", "llm ", "", "two  spaces", "stable-diffusion"] {
            let t = topic(&format!(
                "id = \"ml\"\nname = \"ML\"\nterms = [\"{value}\"]\n"
            ));
            assert!(
                t.problems()
                    .iter()
                    .any(|p| p.contains("must be lowercase words")),
                "`{value}`: {:?}",
                t.problems()
            );
        }
    }

    #[test]
    fn unknown_field_is_rejected() {
        assert!(toml::from_str::<Topic>("id = \"ml\"\nname = \"ML\"\nextra = 1\n").is_err());
    }
}
