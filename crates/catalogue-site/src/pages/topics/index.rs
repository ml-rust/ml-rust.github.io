//! `/topics/`: every job with the jobs below it, then every category.

use askama::Template;
use catalogue_core::{Catalogue, TierReport};

use crate::pages::layout::Layout;
use crate::pages::shared::{CategoryCard, TopicCard, category_cards, topic_cards};

/// The explore page.
#[derive(Debug, Template)]
#[template(path = "topics.html")]
pub(crate) struct TopicsPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// One tile per top-level topic, in file order.
    pub topics: Vec<TopicCard<'a>>,
    /// One tile per category, in file order.
    pub categories: Vec<CategoryCard<'a>>,
    /// Whether any tile counts a topic below it.
    pub has_subtopics: bool,
}

impl<'a> TopicsPage<'a> {
    /// The explore page in `layout`, ordering each tile's names with `report` when given.
    pub(crate) fn new(
        layout: Layout,
        catalogue: &'a Catalogue,
        report: Option<&TierReport>,
    ) -> Self {
        let topics = topic_cards(catalogue, report);
        Self {
            layout,
            has_subtopics: topics.iter().any(|card| !card.subtopics.is_empty()),
            topics,
            categories: category_cards(catalogue),
        }
    }
}
