//! One tier change as a row: date, project link, old tier and new tier.

use std::cmp::Reverse;

use catalogue_core::Catalogue;

use super::labels::tier_name;
use super::paths::project_href;
use crate::href::SafeHref;
use crate::inputs::TierChangeRow;

/// The old-tier text of an entry's first check.
const FIRST_CHECK: &str = "First check";

/// The data one change row of a template reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangeRow<'a> {
    /// The date of the check.
    pub date: String,
    /// The entry name, or the slug of an entry the catalogue no longer holds.
    pub name: &'a str,
    /// The link to the entry page. `None` for an entry the catalogue no longer holds.
    pub href: Option<SafeHref>,
    /// The old tier, or [`FIRST_CHECK`].
    pub from: &'static str,
    /// The new tier.
    pub to: &'static str,
}

/// The rows of `changes`, newest first, at most `limit` of them.
///
/// Changes on the same date keep their input order.
pub(crate) fn change_rows<'a>(
    catalogue: &'a Catalogue,
    changes: &'a [TierChangeRow],
    limit: usize,
) -> Vec<ChangeRow<'a>> {
    let mut sorted: Vec<&TierChangeRow> = changes.iter().collect();
    sorted.sort_by_key(|change| Reverse(change.date));
    sorted
        .into_iter()
        .take(limit)
        .map(|change| {
            let entry = catalogue.entry(&change.slug);
            ChangeRow {
                date: change.date.to_string(),
                name: entry.map_or(change.slug.as_str(), |entry| entry.name.as_str()),
                href: entry.map(|_| project_href(&change.slug)),
                from: change.from.map_or(FIRST_CHECK, tier_name),
                to: tier_name(change.to),
            }
        })
        .collect()
}
