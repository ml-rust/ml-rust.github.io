//! `/changes/`: movement over the window, then every tier change, newest first.

use askama::Template;
use catalogue_core::amount_text::rounded;
use catalogue_core::{Catalogue, MeasuredMovement, Movement, Slug, TierReport, UsersChange};

use super::layout::Layout;
use super::shared::{
    ChangeRow, FirstCheckLine, NamedLink, change_rows, entry_link, first_check_line, movement_note,
    order_text, strongest_first,
};
use crate::inputs::SiteInputs;

/// The tier change page.
#[derive(Debug, Template)]
#[template(path = "changes.html")]
pub(crate) struct ChangesPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// Whether a report gives the tiers.
    pub tiered: bool,
    /// Every tier change after the first check, newest first. Empty without a report.
    pub changes: Vec<ChangeRow<'a>>,
    /// The line that stands for the first check. `None` without a report.
    pub first_check: Option<FirstCheckLine>,
    /// The movement sections. `None` when the history cannot measure movement.
    pub movement: Option<MovementSection<'a>>,
    /// The line shown when the history cannot measure movement.
    pub movement_note: Option<&'static str>,
}

/// The sections of `changes.html` above the tier change table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MovementSection<'a> {
    /// The date of the newest snapshot.
    pub newest: String,
    /// The date of the snapshot nearest the window start.
    pub baseline: String,
    /// The sentence that states the order of each list.
    pub order: &'static str,
    /// The entries first seen inside the window.
    pub new: Vec<FirstSeenRow<'a>>,
    /// The entries whose active users rose.
    pub rising: Vec<UsersRow<'a>>,
    /// The entries whose active users fell.
    pub falling: Vec<UsersRow<'a>>,
}

/// One entry first seen inside the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FirstSeenRow<'a> {
    /// The link to the entry.
    pub entry: NamedLink<'a>,
    /// The date of the entry's oldest snapshot.
    pub date: String,
}

/// One entry whose active users changed enough to count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsersRow<'a> {
    /// The link to the entry.
    pub entry: NamedLink<'a>,
    /// The active users in the baseline snapshot.
    pub from: String,
    /// The active users in the newest snapshot.
    pub to: String,
}

impl<'a> MovementSection<'a> {
    /// The sections of `movement`. An entry the catalogue no longer holds has no row.
    fn new(
        catalogue: &'a Catalogue,
        report: Option<&TierReport>,
        movement: &'a MeasuredMovement,
    ) -> Self {
        let new_slugs = movement.new.iter().map(|found| &found.slug);
        let new = strongest_first(catalogue, report, new_slugs)
            .into_iter()
            .filter_map(|slug| {
                let found = movement.new.iter().find(|found| &found.slug == slug)?;
                Some(FirstSeenRow {
                    entry: entry_link(catalogue, slug)?,
                    date: found.date.to_string(),
                })
            })
            .collect();
        Self {
            newest: movement.newest.to_string(),
            baseline: movement.baseline.to_string(),
            order: order_text(report),
            new,
            rising: users_rows(catalogue, report, &movement.rising),
            falling: users_rows(catalogue, report, &movement.falling),
        }
    }
}

/// The rows of `changes`, in the one order of every list.
fn users_rows<'a>(
    catalogue: &'a Catalogue,
    report: Option<&TierReport>,
    changes: &'a [UsersChange],
) -> Vec<UsersRow<'a>> {
    let slugs: Vec<&Slug> = changes.iter().map(|change| &change.slug).collect();
    strongest_first(catalogue, report, slugs)
        .into_iter()
        .filter_map(|slug| {
            let change = changes.iter().find(|change| &change.slug == slug)?;
            Some(UsersRow {
                entry: entry_link(catalogue, slug)?,
                from: rounded(change.from),
                to: rounded(change.to),
            })
        })
        .collect()
}

impl<'a> ChangesPage<'a> {
    /// The tier change page of `inputs` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>) -> Self {
        let report = inputs.report;
        let changes = match report {
            Some(_) => change_rows(
                inputs.catalogue,
                inputs.changes,
                inputs.first_check,
                inputs.changes.len(),
            ),
            None => Vec::new(),
        };
        let movement = match inputs.movement {
            Movement::Measured(measured) if report.is_some() => {
                Some(MovementSection::new(inputs.catalogue, report, measured))
            }
            _ => None,
        };
        Self {
            layout,
            tiered: report.is_some(),
            changes,
            first_check: report.and_then(|_| first_check_line(inputs.changes, inputs.first_check)),
            movement,
            movement_note: movement_note(inputs),
        }
    }
}
