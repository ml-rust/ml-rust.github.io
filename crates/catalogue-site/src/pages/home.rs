//! `/`: what the catalogue is, the topic grid, the categories, Featured entries, new entries and
//! tier changes.

use std::cmp::Reverse;

use askama::Template;

use super::layout::Layout;
use super::shared::{
    ChangeRow, EntryRow, FirstCheckLine, NamedLink, TopicCard, category_links, change_rows,
    featured_rows, first_check_line, order_text, tier_of, topic_cards,
};
use crate::inputs::SiteInputs;

/// How many recently added entries and recent tier changes the home page lists.
const RECENT: usize = 10;

/// How many project families the Featured list of the home page shows.
const FEATURED_FAMILIES: usize = 8;

/// The home page.
#[derive(Debug, Template)]
#[template(path = "index.html")]
pub(crate) struct HomePage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// The run date of the check behind the tiers. `None` without a report.
    pub checked: Option<String>,
    /// One card per top-level topic, in file order.
    pub topics: Vec<TopicCard<'a>>,
    /// The link to every category, in file order.
    pub categories: Vec<NamedLink<'a>>,
    /// The first Featured project families. Empty without a report.
    pub featured: Vec<EntryRow<'a>>,
    /// The sentence that states the order of the Featured list.
    pub order: &'static str,
    /// The entries added last, newest first.
    pub recent: Vec<EntryRow<'a>>,
    /// The latest tier changes, newest first. Empty without a report.
    pub changes: Vec<ChangeRow<'a>>,
    /// The line that stands for the first check. `None` without a report.
    pub first_check: Option<FirstCheckLine>,
}

impl<'a> HomePage<'a> {
    /// The home page of `inputs` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>) -> Self {
        let catalogue = inputs.catalogue;
        let report = inputs.report;
        let mut featured = featured_rows(inputs, EntryRow::new);
        featured.truncate(FEATURED_FAMILIES);
        let mut added: Vec<_> = catalogue.entries().iter().collect();
        added.sort_by_cached_key(|(slug, entry)| {
            (Reverse(entry.added), entry.name.to_lowercase(), *slug)
        });
        let recent = added
            .into_iter()
            .take(RECENT)
            .map(|(slug, entry)| {
                EntryRow::new(slug, entry, tier_of(report, slug)).with_added(entry)
            })
            .collect();
        Self {
            layout,
            checked: report.map(|report| report.generated.to_string()),
            topics: topic_cards(catalogue, report),
            categories: category_links(catalogue),
            featured,
            order: order_text(report),
            recent,
            changes: report
                .map(|_| change_rows(catalogue, inputs.changes, inputs.first_check, RECENT))
                .unwrap_or_default(),
            first_check: report.and_then(|_| first_check_line(inputs.changes, inputs.first_check)),
        }
    }
}
