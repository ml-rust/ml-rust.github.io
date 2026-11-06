//! The fixed list of what a team needs from each topic.

use catalogue_core::Slug;
use serde::Deserialize;

/// The list, compiled in so the VM needs no checkout.
const NEEDS: &str = include_str!("needs.toml");

/// The needs of every topic.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeedList {
    /// One table per topic.
    pub topic: Vec<TopicNeeds>,
}

/// The needs of one topic.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopicNeeds {
    /// The topic id.
    pub id: Slug,
    /// The job the topic stands for.
    pub job: String,
    /// The needs, in report order.
    pub needs: Vec<Need>,
}

/// One need.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Need {
    /// The id a model answers with.
    pub id: String,
    /// What the need asks, as the page prints it.
    pub ask: String,
}

/// The compiled-in list.
///
/// # Panics
///
/// Never at run time: a test parses the same text.
pub fn need_list() -> NeedList {
    toml::from_str(NEEDS).unwrap_or_else(|error| panic!("needs.toml: {error}"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_topic_of_the_data_has_needs_and_no_other_topic_does() {
        let topics: toml::Table =
            toml::from_str(include_str!("../../../data/topics.toml")).expect("topics");
        let expected: BTreeSet<String> = topics["topic"]
            .as_array()
            .expect("array")
            .iter()
            .filter(|topic| topic.get("parent").is_none())
            .map(|topic| topic["id"].as_str().expect("id").to_owned())
            .collect();
        let found: BTreeSet<String> = need_list()
            .topic
            .iter()
            .map(|topic| topic.id.to_string())
            .collect();
        assert_eq!(found, expected);
    }

    #[test]
    fn need_ids_are_unique_within_a_topic() {
        for topic in need_list().topic {
            let ids: BTreeSet<&str> = topic.needs.iter().map(|need| need.id.as_str()).collect();
            assert_eq!(ids.len(), topic.needs.len(), "{}", topic.id);
            assert!(topic.needs.len() >= 4, "{}", topic.id);
        }
    }
}
