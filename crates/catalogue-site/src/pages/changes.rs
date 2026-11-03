//! `/changes/`: every tier change, newest first.

use askama::Template;

use super::layout::Layout;
use super::shared::{ChangeRow, change_rows};
use crate::inputs::SiteInputs;

/// The tier change page.
#[derive(Debug, Template)]
#[template(path = "changes.html")]
pub(crate) struct ChangesPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// Whether a report gives the tiers.
    pub tiered: bool,
    /// Every tier change, newest first. Empty without a report.
    pub changes: Vec<ChangeRow<'a>>,
}

impl<'a> ChangesPage<'a> {
    /// The tier change page of `inputs` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>) -> Self {
        let changes = match inputs.report {
            Some(_) => change_rows(inputs.catalogue, inputs.changes, inputs.changes.len()),
            None => Vec::new(),
        };
        Self {
            layout,
            tiered: inputs.report.is_some(),
            changes,
        }
    }
}
