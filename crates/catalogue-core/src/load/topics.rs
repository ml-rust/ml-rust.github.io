use std::collections::{BTreeMap, BTreeSet};

use super::diagnostics::Diagnostics;
use super::graph::reaches;
use super::parse::Parsed;
use crate::ids::Slug;
use crate::registry::{Topic, Topics};

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
    check_mappings(topics, file, diags);
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

/// Reports a keyword or category that two topics claim, or that one topic lists twice.
fn check_mappings(topics: &[Topic], file: &Parsed<Topics>, diags: &mut Diagnostics) {
    let mut keywords: BTreeMap<&str, &Slug> = BTreeMap::new();
    let mut categories: BTreeMap<&str, &Slug> = BTreeMap::new();
    for topic in topics {
        claim(
            "keyword",
            &topic.keywords,
            topic,
            &mut keywords,
            file,
            diags,
        );
        claim(
            "category",
            &topic.categories,
            topic,
            &mut categories,
            file,
            diags,
        );
    }
}

fn claim<'a>(
    what: &str,
    values: &'a [String],
    topic: &'a Topic,
    owners: &mut BTreeMap<&'a str, &'a Slug>,
    file: &Parsed<Topics>,
    diags: &mut Diagnostics,
) {
    for value in values {
        match owners.insert(value.as_str(), &topic.id) {
            Some(first) if *first == topic.id => diags.push(
                &file.path,
                format!("topic `{}`: {what} `{value}` is listed twice", topic.id),
            ),
            Some(first) => diags.push(
                &file.path,
                format!(
                    "{what} `{value}` maps to topics `{first}` and `{}`; a {what} maps to one topic",
                    topic.id
                ),
            ),
            None => {}
        }
    }
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
    fn a_keyword_maps_to_one_topic() {
        let found = with_topics(
            "keywords = [\"llm\"]\n[[topic]]\nid = \"gpu\"\nname = \"GPU\"\nkeywords = [\"llm\"]\n",
        );
        assert!(
            found
                .iter()
                .any(|m| m.contains("keyword `llm` maps to topics `tensors` and `gpu`")),
            "{found:?}"
        );
    }

    #[test]
    fn a_category_maps_to_one_topic() {
        let found = with_topics(
            "categories = [\"science::ml\"]\n[[topic]]\nid = \"gpu\"\nname = \"GPU\"\ncategories = [\"science::ml\"]\n",
        );
        assert!(
            found
                .iter()
                .any(|m| m.contains("category `science::ml` maps to topics `tensors` and `gpu`")),
            "{found:?}"
        );
    }

    #[test]
    fn a_repeated_keyword_in_one_topic_is_reported() {
        let found = with_topics("keywords = [\"llm\", \"llm\"]\n");
        assert!(
            found
                .iter()
                .any(|m| m.contains("keyword `llm` is listed twice")),
            "{found:?}"
        );
    }

    #[test]
    fn distinct_mappings_pass() {
        assert!(
            with_topics(
                "keywords = [\"llm\"]\ncategories = [\"science\"]\n[[topic]]\nid = \"gpu\"\nname = \"GPU\"\nkeywords = [\"cuda\"]\ncategories = [\"science::ml\"]\n"
            )
            .is_empty()
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
