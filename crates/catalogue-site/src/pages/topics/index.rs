//! `/topics/`: every top-level topic with the topics below it.

use askama::Template;
use catalogue_core::{Catalogue, TierReport};

use crate::pages::layout::Layout;
use crate::pages::shared::{TopicCard, topic_cards};

/// The topic list page.
#[derive(Debug, Template)]
#[template(path = "topics.html")]
pub(crate) struct TopicsPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// One card per top-level topic, in file order.
    pub topics: Vec<TopicCard<'a>>,
}

impl<'a> TopicsPage<'a> {
    /// The topic list in `layout`, with tier counts from `report` when given.
    pub(crate) fn new(
        layout: Layout,
        catalogue: &'a Catalogue,
        report: Option<&TierReport>,
    ) -> Self {
        Self {
            layout,
            topics: topic_cards(catalogue, report),
        }
    }
}
