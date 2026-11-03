//! `/`: what the catalogue is, the topic grid, Featured entries, new entries and tier changes.

use std::cmp::Reverse;

use askama::Template;
use catalogue_core::Tier;

use super::layout::Layout;
use super::shared::{
    ChangeRow, EntryRow, TopicCard, change_rows, sort_by_name, tier_of, topic_cards,
};
use crate::inputs::SiteInputs;

/// How many recently added entries and recent tier changes the home page lists.
const RECENT: usize = 10;

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
    /// Every Featured entry, in name order. Empty without a report.
    pub featured: Vec<EntryRow<'a>>,
    /// The entries added last, newest first.
    pub recent: Vec<EntryRow<'a>>,
    /// The latest tier changes, newest first. Empty without a report.
    pub changes: Vec<ChangeRow<'a>>,
}

impl<'a> HomePage<'a> {
    /// The home page of `inputs` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>) -> Self {
        let catalogue = inputs.catalogue;
        let report = inputs.report;
        let mut featured: Vec<EntryRow<'a>> = catalogue
            .entries()
            .iter()
            .filter(|(slug, _)| tier_of(report, slug) == Some(Tier::Featured))
            .map(|(slug, entry)| EntryRow::new(slug, entry, Some(Tier::Featured)))
            .collect();
        sort_by_name(&mut featured);
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
            featured,
            recent,
            changes: report
                .map(|_| change_rows(catalogue, inputs.changes, RECENT))
                .unwrap_or_default(),
        }
    }
}
