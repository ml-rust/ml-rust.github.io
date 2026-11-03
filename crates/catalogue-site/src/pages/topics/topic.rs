//! `/topics/<id>/`: one topic with its summary, subtopics and entries.

use askama::Template;
use catalogue_core::{Catalogue, TierReport, Topic};

use crate::pages::layout::Layout;
use crate::pages::shared::{
    EntryGroup, EvidenceLink, NamedLink, group_entries, topic_href, topic_link,
};

/// The page of one topic.
#[derive(Debug, Template)]
#[template(path = "topic.html")]
pub(crate) struct TopicPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// The topic name.
    pub name: &'a str,
    /// The broader topic.
    pub parent: Option<NamedLink<'a>>,
    /// The summary text, if the topic has one.
    pub summary: Option<&'a str>,
    /// The links that support the summary.
    pub evidence: Vec<EvidenceLink>,
    /// The topics directly below this one, in file order.
    pub children: Vec<NamedLink<'a>>,
    /// The entries of the topic, grouped by tier with a report.
    pub groups: Vec<EntryGroup<'a>>,
    /// Whether a report gives the tiers.
    pub tiered: bool,
}

impl<'a> TopicPage<'a> {
    /// The page of `topic` in `layout`, with tiers from `report` when given.
    pub(crate) fn new(
        layout: Layout,
        catalogue: &'a Catalogue,
        report: Option<&TierReport>,
        topic: &'a Topic,
    ) -> Self {
        let slugs = catalogue.entries_in_topic(&topic.id);
        let children = catalogue
            .topics()
            .iter()
            .filter(|child| child.parent.as_ref() == Some(&topic.id))
            .map(|child| NamedLink {
                name: &child.name,
                href: topic_href(&child.id),
            })
            .collect();
        Self {
            layout,
            name: &topic.name,
            parent: topic
                .parent
                .as_ref()
                .and_then(|parent| topic_link(catalogue, parent)),
            summary: topic.summary.as_ref().map(|summary| summary.text.as_str()),
            evidence: EvidenceLink::of_summary(topic),
            children,
            groups: group_entries(catalogue, report, &slugs),
            tiered: report.is_some(),
        }
    }
}
