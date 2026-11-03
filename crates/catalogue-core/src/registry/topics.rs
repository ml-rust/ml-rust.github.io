use serde::{Deserialize, Serialize};
use url::Url;

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
        }
        out
    }
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
    fn unknown_field_is_rejected() {
        assert!(toml::from_str::<Topic>("id = \"ml\"\nname = \"ML\"\nextra = 1\n").is_err());
    }
}
