//! Entries grouped by tier, or one alphabetical list without a report.

use catalogue_core::{Catalogue, Entry, Slug, Tier, TierReport};

use super::entry_row::{EntryRow, sort_by_name, tier_of};
use super::labels::tier_name;

/// The heading of the group for entries that the report holds no record for.
const NOT_CHECKED: &str = "Not checked";

/// One group of entry rows, as `topic.html` reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryGroup<'a> {
    /// The group heading. `None` for the one list of a build without a report.
    pub heading: Option<&'static str>,
    /// The rows, in name order.
    pub rows: Vec<EntryRow<'a>>,
}

/// The entries `slugs`, grouped for display.
///
/// With a report: one group per tier, from Featured to Archived, with no empty
/// group. Without a report: one group with no heading. Rows are in name order.
pub(crate) fn group_entries<'a>(
    catalogue: &'a Catalogue,
    report: Option<&TierReport>,
    slugs: &[&Slug],
) -> Vec<EntryGroup<'a>> {
    group_rows(catalogue, report, slugs, EntryRow::new)
}

/// The groups of [`group_entries`], with each row leading with how to get the entry.
pub(crate) fn group_install_rows<'a>(
    catalogue: &'a Catalogue,
    report: Option<&TierReport>,
    slugs: &[&Slug],
) -> Vec<EntryGroup<'a>> {
    group_rows(catalogue, report, slugs, |slug, entry, tier| {
        EntryRow::new(slug, entry, tier).with_install(entry)
    })
}

fn group_rows<'a>(
    catalogue: &'a Catalogue,
    report: Option<&TierReport>,
    slugs: &[&Slug],
    row: impl Fn(&Slug, &'a Entry, Option<Tier>) -> EntryRow<'a>,
) -> Vec<EntryGroup<'a>> {
    let mut rows: Vec<(Option<Tier>, EntryRow<'a>)> = slugs
        .iter()
        .filter_map(|&slug| {
            let entry = catalogue.entry(slug)?;
            let tier = tier_of(report, slug);
            Some((tier, row(slug, entry, tier)))
        })
        .collect();
    if report.is_none() {
        let mut all: Vec<EntryRow<'a>> = rows.into_iter().map(|(_, row)| row).collect();
        sort_by_name(&mut all);
        return if all.is_empty() {
            Vec::new()
        } else {
            vec![EntryGroup {
                heading: None,
                rows: all,
            }]
        };
    }
    let mut groups = Vec::new();
    for heading_tier in Tier::ALL.into_iter().map(Some).chain([None]) {
        let (taken, rest): (Vec<_>, Vec<_>) = rows
            .into_iter()
            .partition(|(tier, _)| *tier == heading_tier);
        rows = rest;
        let mut group: Vec<EntryRow<'a>> = taken.into_iter().map(|(_, row)| row).collect();
        if group.is_empty() {
            continue;
        }
        sort_by_name(&mut group);
        groups.push(EntryGroup {
            heading: Some(heading_tier.map_or(NOT_CHECKED, tier_name)),
            rows: group,
        });
    }
    groups
}
