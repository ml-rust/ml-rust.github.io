use std::collections::{BTreeMap, BTreeSet};

use super::diagnostics::Diagnostics;
use super::graph::reaches;
use super::parse::Parsed;
use crate::ids::Slug;
use crate::registry::Topics;

/// Checks the topic list and returns the set of topic ids.
pub(crate) fn check_topics(file: &Parsed<Topics>, diags: &mut Diagnostics) -> BTreeSet<Slug> {
    let topics = &file.value.topic;
    let mut ids: BTreeSet<Slug> = BTreeSet::new();
    for topic in topics {
        for problem in topic.problems() {
            diags.push(&file.path, format!("topic `{}`: {problem}", topic.id));
        }
        if !ids.insert(topic.id.clone()) {
            diags.push(&file.path, format!("topic `{}` is defined twice", topic.id));
        }
    }
    let mut parents: BTreeMap<Slug, Vec<Slug>> = BTreeMap::new();
    for topic in topics {
        let Some(parent) = &topic.parent else {
            continue;
        };
        if ids.contains(parent) {
            parents
                .entry(topic.id.clone())
                .or_default()
                .push(parent.clone());
        } else {
            diags.push(
                &file.path,
                format!("topic `{}`: parent `{parent}` does not exist", topic.id),
            );
        }
    }
    for topic in topics {
        let Some(parent) = &topic.parent else {
            continue;
        };
        if reaches(&parents, parent, &topic.id) {
            diags.push(
                &file.path,
                format!("topic `{}`: parent chain forms a cycle", topic.id),
            );
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use crate::testing::{errors, expect_error, put, valid};

    const TOPICS: &str = "[[topic]]\nid = \"tensors\"\nname = \"Tensors\"\n";

    fn with_topics(extra: &str) -> Vec<String> {
        let mut src = valid();
        put(&mut src, "topics.toml", &format!("{TOPICS}{extra}"));
        errors(src)
    }

    #[test]
    fn nested_topics_pass() {
        assert!(
            with_topics("[[topic]]\nid = \"gpu\"\nname = \"GPU\"\nparent = \"tensors\"\n")
                .is_empty()
        );
    }

    #[test]
    fn ids_must_be_unique() {
        let found = with_topics("[[topic]]\nid = \"tensors\"\nname = \"Again\"\n");
        assert!(
            found.iter().any(|m| m.contains("defined twice")),
            "{found:?}"
        );
    }

    #[test]
    fn parent_must_exist() {
        let found = with_topics("[[topic]]\nid = \"gpu\"\nname = \"GPU\"\nparent = \"ghost\"\n");
        assert!(
            found
                .iter()
                .any(|m| m.contains("parent `ghost` does not exist")),
            "{found:?}"
        );
    }

    #[test]
    fn parent_chain_must_not_cycle() {
        let found = with_topics(
            "[[topic]]\nid = \"a\"\nname = \"A\"\nparent = \"b\"\n[[topic]]\nid = \"b\"\nname = \"B\"\nparent = \"a\"\n",
        );
        assert_eq!(
            found.iter().filter(|m| m.contains("forms a cycle")).count(),
            2,
            "{found:?}"
        );
    }

    #[test]
    fn a_topic_cannot_be_its_own_parent() {
        let found = with_topics("[[topic]]\nid = \"a\"\nname = \"A\"\nparent = \"a\"\n");
        assert!(
            found.iter().any(|m| m.contains("forms a cycle")),
            "{found:?}"
        );
    }

    #[test]
    fn per_topic_rules_are_reported() {
        let mut src = valid();
        put(
            &mut src,
            "topics.toml",
            "[[topic]]\nid = \"tensors\"\nname = \"\"\n",
        );
        expect_error(src, "name must not be empty");
    }
}
