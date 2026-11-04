//! `/featured/`: every Featured project family.

use askama::Template;

use super::layout::Layout;
use super::shared::{EntryRow, featured_rows, order_text};
use crate::inputs::SiteInputs;

/// The full Featured list.
#[derive(Debug, Template)]
#[template(path = "featured.html")]
pub(crate) struct FeaturedPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// Whether a report gives the tiers.
    pub tiered: bool,
    /// The sentence that states the order of the list.
    pub order: &'static str,
    /// One row per family. Empty without a report.
    pub rows: Vec<EntryRow<'a>>,
}

impl<'a> FeaturedPage<'a> {
    /// The Featured list of `inputs` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>) -> Self {
        let report = inputs.report;
        Self {
            layout,
            tiered: report.is_some(),
            order: order_text(report),
            rows: featured_rows(inputs, |slug, entry, tier| {
                let record = report.and_then(|report| report.entries.get(slug));
                EntryRow::new(slug, entry, tier).with_choice(record)
            }),
        }
    }
}
