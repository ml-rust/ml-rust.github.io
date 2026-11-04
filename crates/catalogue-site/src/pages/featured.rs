//! `/featured/`: every Featured project family.

use askama::Template;

use super::layout::Layout;
use super::shared::{
    EntryGroup, FEATURED_MEANING, TierLine, checked_dates, featured_order_text, featured_rows,
    featured_slugs, movement_note, project_count, tier_lines,
};
use crate::inputs::SiteInputs;

/// The full Featured list.
#[derive(Debug, Template)]
#[template(path = "featured.html")]
pub(crate) struct FeaturedPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// Whether a report gives the tiers.
    pub tiered: bool,
    /// The sentence that says what Featured means.
    pub meaning: &'static str,
    /// The project count as text.
    pub count: String,
    /// The date of the check as an ISO date. `None` without a report.
    pub checked: Option<String>,
    /// The date of the check as a person reads it. `None` without a report.
    pub checked_text: Option<String>,
    /// The sentence that states the order of the list.
    pub order: &'static str,
    /// One group of one row per family. Empty without a Featured row.
    pub groups: Vec<EntryGroup<'a>>,
    /// Every tier with its meaning.
    pub tiers: Vec<TierLine>,
    /// The line shown when the history cannot measure movement.
    pub movement_note: Option<&'static str>,
}

impl<'a> FeaturedPage<'a> {
    /// The Featured list of `inputs` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>) -> Self {
        let rows = featured_rows(inputs);
        let (checked, checked_text) = checked_dates(inputs);
        Self {
            layout,
            tiered: inputs.report.is_some(),
            meaning: FEATURED_MEANING,
            count: project_count(featured_slugs(inputs).len()),
            checked,
            checked_text,
            order: featured_order_text(),
            groups: if rows.is_empty() {
                Vec::new()
            } else {
                vec![EntryGroup::plain(rows)]
            },
            tiers: tier_lines(),
            movement_note: movement_note(inputs),
        }
    }

    /// Whether the page holds a table for the list script.
    pub(crate) fn uses_list_script(&self) -> bool {
        !self.groups.is_empty()
    }
}
