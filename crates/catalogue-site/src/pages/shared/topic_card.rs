//! The topic grid: one card per top-level topic, with every topic below it.

use std::collections::BTreeSet;

use catalogue_core::{Catalogue, Slug, Tier, TierReport, Topic};

use super::entry_row::tier_of;
use super::labels::tier_name;
use super::paths::{EvidenceLink, NamedLink, topic_href};
use crate::href::SafeHref;

/// One card of `topic_grid.html`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopicCard<'a> {
    /// The topic name.
    pub name: &'a str,
    /// The link to the topic page.
    pub href: SafeHref,
    /// The summary text, if the topic has one.
    pub summary: Option<&'a str>,
    /// The links that support the summary.
    pub evidence: Vec<EvidenceLink>,
    /// The entry count of each tier. Empty without a report.
    pub counts: Vec<TierCount>,
    /// The entry count as text, such as `3 entries`. `None` with a report.
    pub entries: Option<String>,
    /// Every topic below this one, depth first, in file order.
    pub subtopics: Vec<NamedLink<'a>>,
}

/// The number of entries in one tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TierCount {
    /// The tier name.
    pub name: &'static str,
    /// The number of entries.
    pub count: usize,
}

/// One card per top-level topic, in file order.
///
/// A card counts the entries of its topic and of every topic below it, each entry once.
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

/// The card of the top-level topic `topic`.
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
    let counts = match report {
        Some(_) => Tier::ALL
            .into_iter()
            .map(|tier| TierCount {
                name: tier_name(tier),
                count: slugs
                    .iter()
                    .filter(|slug| tier_of(report, slug) == Some(tier))
                    .count(),
            })
            .collect(),
        None => Vec::new(),
    };
    TopicCard {
        name: &topic.name,
        href: topic_href(&topic.id),
        summary: topic.summary.as_ref().map(|summary| summary.text.as_str()),
        evidence: EvidenceLink::of_summary(topic),
        counts,
        entries: report.is_none().then(|| entry_count(slugs.len())),
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

/// `count` entries as text: `1 entry`, `3 entries`.
fn entry_count(count: usize) -> String {
    if count == 1 {
        "1 entry".to_owned()
    } else {
        format!("{count} entries")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_count_text_names_one_entry_in_the_singular() {
        assert_eq!(entry_count(0), "0 entries");
        assert_eq!(entry_count(1), "1 entry");
        assert_eq!(entry_count(3), "3 entries");
    }
}
