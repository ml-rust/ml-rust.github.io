//! The job tiles: one per top-level topic, with its count and the top project names.

use std::collections::BTreeSet;

use catalogue_core::{Catalogue, Slug, TierReport, Topic};

use super::listing::project_count;
use super::order::family_heads;
use super::paths::{EvidenceLink, NamedLink, topic_href};
use crate::href::SafeHref;

/// How many project names a tile shows.
const TOP_NAMES: usize = 2;

/// One tile of `job_tile.html`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopicCard<'a> {
    /// The topic id. It picks the icon.
    pub id: String,
    /// The topic name.
    pub name: &'a str,
    /// The link to the topic page.
    pub href: SafeHref,
    /// The summary text, if the topic has one.
    pub summary: Option<&'a str>,
    /// The links that support the summary.
    pub evidence: Vec<EvidenceLink>,
    /// The project count as text. It covers the topic and every topic below it.
    pub count: String,
    /// The names of the first projects of the one list order, joined by commas.
    pub top: String,
    /// Every topic below this one, depth first, in file order.
    pub subtopics: Vec<NamedLink<'a>>,
}

/// One tile per top-level topic, in file order.
///
/// A tile counts the entries of its topic and of every topic below it, each entry once.
pub(crate) fn topic_cards<'a>(
    catalogue: &'a Catalogue,
    report: Option<&TierReport>,
) -> Vec<TopicCard<'a>> {
    catalogue
        .topics()
        .iter()
        .filter(|topic| topic.parent.is_none())
        .map(|topic| card(catalogue, report, topic))
        .collect()
}

/// The tile of the top-level topic `topic`.
fn card<'a>(
    catalogue: &'a Catalogue,
    report: Option<&TierReport>,
    topic: &'a Topic,
) -> TopicCard<'a> {
    let tree = subtree(catalogue.topics(), topic);
    let slugs: BTreeSet<&Slug> = tree
        .iter()
        .flat_map(|member| catalogue.entries_in_topic(&member.id))
        .collect();
    let listed: Vec<&Slug> = slugs.iter().copied().collect();
    TopicCard {
        id: topic.id.as_str().to_owned(),
        name: &topic.name,
        href: topic_href(&topic.id),
        summary: topic.summary.as_ref().map(|summary| summary.text.as_str()),
        evidence: EvidenceLink::of_summary(topic),
        count: project_count(slugs.len()),
        top: top_names(catalogue, report, &listed),
        subtopics: tree
            .iter()
            .skip(1)
            .map(|member| NamedLink {
                name: &member.name,
                href: topic_href(&member.id),
            })
            .collect(),
    }
}

/// The names of the first [`TOP_NAMES`] rows of a list of `slugs`, joined by commas.
pub(crate) fn top_names(
    catalogue: &Catalogue,
    report: Option<&TierReport>,
    slugs: &[&Slug],
) -> String {
    let names: Vec<&str> = family_heads(catalogue, report, slugs)
        .into_iter()
        .filter_map(|head| catalogue.entry(head.slug))
        .take(TOP_NAMES)
        .map(|entry| entry.name.as_str())
        .collect();
    names.join(", ")
}

/// `root` and then every topic below it, depth first, in file order.
fn subtree<'a>(topics: &'a [Topic], root: &'a Topic) -> Vec<&'a Topic> {
    let mut out = vec![root];
    let mut seen: BTreeSet<&Slug> = BTreeSet::from([&root.id]);
    visit(topics, root, &mut seen, &mut out);
    out
}

/// Pushes every topic below `node` onto `out`, depth first, skipping topics in `seen`.
fn visit<'a>(
    topics: &'a [Topic],
    node: &'a Topic,
    seen: &mut BTreeSet<&'a Slug>,
    out: &mut Vec<&'a Topic>,
) {
    for child in topics
        .iter()
        .filter(|topic| topic.parent.as_ref() == Some(&node.id))
    {
        if seen.insert(&child.id) {
            out.push(child);
            visit(topics, child, seen, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::Tier;

    use super::*;
    use crate::testing::{catalogue, entry, record, report, slug};

    fn topic(id: &str, parent: Option<&str>) -> Topic {
        Topic {
            id: Slug::new(id).expect("slug"),
            name: id.to_uppercase(),
            parent: parent.map(|parent| Slug::new(parent).expect("slug")),
            summary: None,
            keywords: Vec::new(),
            categories: Vec::new(),
            terms: Vec::new(),
        }
    }

    #[test]
    fn the_subtree_is_depth_first_in_file_order() {
        let topics = [
            topic("ml", None),
            topic("tensors", Some("ml")),
            topic("vision", Some("ml")),
            topic("gpu", Some("tensors")),
            topic("audio", None),
        ];
        let ids: Vec<&str> = subtree(&topics, &topics[0])
            .into_iter()
            .map(|topic| topic.id.as_str())
            .collect();
        assert_eq!(ids, ["ml", "tensors", "gpu", "vision"]);
        assert_eq!(subtree(&topics, &topics[4]).len(), 1);
    }

    #[test]
    fn the_top_names_follow_the_list_order_and_stop_at_two() {
        let data = catalogue(vec![
            entry("alpha", ""),
            entry("beta", ""),
            entry("gamma", ""),
        ]);
        let tiers = report(vec![
            ("alpha", record(Tier::Listed, None)),
            ("beta", record(Tier::Featured, None)),
            ("gamma", record(Tier::Archived, None)),
        ]);
        let (alpha, beta, gamma) = (slug("alpha"), slug("beta"), slug("gamma"));
        let found = top_names(&data, Some(&tiers), &[&alpha, &beta, &gamma]);
        let first = data.entry(&beta).expect("entry").name.clone();
        let second = data.entry(&alpha).expect("entry").name.clone();
        assert_eq!(found, format!("{first}, {second}"));
    }
}
