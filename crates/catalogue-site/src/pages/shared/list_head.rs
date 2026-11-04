//! What every list page shows above its rows: count, start here, tier links and tier meanings.

use super::date_text::date_text;
use super::entry_row::EntryRow;
use super::listing::{EntryGroup, project_count};
use super::movement::movement_note;
use super::order::order_text;
use super::tier_help::{TierLine, tier_lines};
use crate::inputs::SiteInputs;

/// How many Featured rows the start here highlight shows.
const START_HERE: usize = 3;

/// The class of the Featured tier, as [`EntryGroup::tier_class`] holds it.
const FEATURED_CLASS: &str = "tier-featured";

/// The parts above the rows of a job or category page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListHead<'a> {
    /// The project count as text.
    pub count: String,
    /// The first Featured rows. Empty when the list holds no Featured row.
    pub start: Vec<EntryRow<'a>>,
    /// The date of the check as an ISO date. `None` without a report.
    pub checked: Option<String>,
    /// The date of the check as a person reads it. `None` without a report.
    pub checked_text: Option<String>,
    /// The sentence that states the order of the list.
    pub order: &'static str,
    /// The line shown when the history cannot measure movement.
    pub movement_note: Option<&'static str>,
    /// Every tier with its meaning.
    pub tiers: Vec<TierLine>,
}

impl<'a> ListHead<'a> {
    /// The head of a list of `total` entries shown as `groups`.
    pub(crate) fn new(inputs: &SiteInputs<'a>, total: usize, groups: &[EntryGroup<'a>]) -> Self {
        let (checked, checked_text) = checked_dates(inputs);
        Self {
            count: project_count(total),
            start: groups
                .iter()
                .find(|group| group.tier_class == Some(FEATURED_CLASS))
                .map(|group| group.rows.iter().take(START_HERE).cloned().collect())
                .unwrap_or_default(),
            checked,
            checked_text,
            order: order_text(inputs.report),
            movement_note: movement_note(inputs),
            tiers: tier_lines(),
        }
    }
}

/// The date of the check as an ISO date and as a person reads it. `None` without a report.
pub(crate) fn checked_dates(inputs: &SiteInputs<'_>) -> (Option<String>, Option<String>) {
    let checked = inputs.report.map(|report| report.generated.to_string());
    let text = checked
        .as_deref()
        .map(|iso| date_text(iso).unwrap_or_else(|| iso.to_owned()));
    (checked, text)
}
