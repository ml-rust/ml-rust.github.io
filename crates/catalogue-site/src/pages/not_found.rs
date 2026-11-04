//! `/404.html`: the page GitHub Pages serves for an unknown path.

use askama::Template;
use catalogue_core::{Catalogue, TierReport};

use super::layout::Layout;
use super::shared::{TopicCard, topic_cards};

/// The not-found page.
#[derive(Debug, Template)]
#[template(path = "404.html")]
pub(crate) struct NotFoundPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// One tile per top-level job, so a person can recover without a search.
    pub topics: Vec<TopicCard<'a>>,
}

impl<'a> NotFoundPage<'a> {
    /// The not-found page in `layout`.
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
