//! `/topics/<id>/`: one job with its summary, subtopics and entries.

use askama::Template;
use catalogue_core::Topic;

use crate::inputs::SiteInputs;
use crate::pages::layout::Layout;
use crate::pages::shared::{
    EntryGroup, EvidenceLink, ListHead, NamedLink, group_entries, topic_href, topic_link,
};

/// The page of one job.
#[derive(Debug, Template)]
#[template(path = "topic.html")]
pub(crate) struct TopicPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// The topic id. It picks the icon.
    pub id: String,
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
    /// The parts above the rows.
    pub head: ListHead<'a>,
}

impl<'a> TopicPage<'a> {
    /// The page of `topic` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>, topic: &'a Topic) -> Self {
        let catalogue = inputs.catalogue;
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
        let groups = group_entries(inputs, &slugs);
        Self {
            layout,
            id: topic.id.as_str().to_owned(),
            name: &topic.name,
            parent: topic
                .parent
                .as_ref()
                .and_then(|parent| topic_link(catalogue, parent)),
            summary: topic.summary.as_ref().map(|summary| summary.text.as_str()),
            evidence: EvidenceLink::of_summary(topic),
            children,
            head: ListHead::new(inputs, slugs.len(), &groups),
            groups,
        }
    }

    /// Whether the page holds a table for the list script.
    pub(crate) fn uses_list_script(&self) -> bool {
        !self.groups.is_empty()
    }
}
