//! Entry lists: one row per project family, in the one order of every list.

use catalogue_core::{Entry, Slug, Tier};

use super::entry_row::{EntryRow, RelatedLink, tier_of};
use super::labels::tier_name;
use super::movement::movement_word;
use super::order::family_heads;
use crate::inputs::SiteInputs;

/// The heading of the group for entries that the report holds no record for.
const NOT_CHECKED: &str = "Not checked";

/// One group of entry rows, as `topic.html` reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryGroup<'a> {
    /// The group heading. `None` for the one list of a build without a report.
    pub heading: Option<&'static str>,
    /// The rows, in the one order of every list.
    pub rows: Vec<EntryRow<'a>>,
}

/// The entries `slugs`, grouped for display.
///
/// With a report: one group per tier, from Featured to Archived, with no empty
/// group. Without a report: one group with no heading.
pub(crate) fn group_entries<'a>(inputs: &SiteInputs<'a>, slugs: &[&Slug]) -> Vec<EntryGroup<'a>> {
    group_rows(inputs, slugs, EntryRow::new)
}

/// The groups of [`group_entries`], with each row leading with how to get the entry.
pub(crate) fn group_install_rows<'a>(
    inputs: &SiteInputs<'a>,
    slugs: &[&Slug],
) -> Vec<EntryGroup<'a>> {
    group_rows(inputs, slugs, |slug, entry, tier| {
        EntryRow::new(slug, entry, tier).with_install(entry)
    })
}

/// One row per family of the Featured entries, in the one order of every list.
pub(crate) fn featured_rows<'a>(
    inputs: &SiteInputs<'a>,
    row: impl Fn(&Slug, &'a Entry, Option<Tier>) -> EntryRow<'a>,
) -> Vec<EntryRow<'a>> {
    let slugs: Vec<&Slug> = inputs
        .catalogue
        .entries()
        .keys()
        .filter(|slug| tier_of(inputs.report, slug) == Some(Tier::Featured))
        .collect();
    family_rows(inputs, &slugs, row)
        .into_iter()
        .map(|(_, row)| row)
        .collect()
}

/// One row per family of `slugs`, each with its tier.
fn family_rows<'a>(
    inputs: &SiteInputs<'a>,
    slugs: &[&Slug],
    row: impl Fn(&Slug, &'a Entry, Option<Tier>) -> EntryRow<'a>,
) -> Vec<(Option<Tier>, EntryRow<'a>)> {
    let catalogue = inputs.catalogue;
    family_heads(catalogue, inputs.report, slugs)
        .into_iter()
        .filter_map(|head| {
            let entry = catalogue.entry(head.slug)?;
            let tier = tier_of(inputs.report, head.slug);
            let found = row(head.slug, entry, tier).with_family(
                movement_word(inputs, head.slug),
                RelatedLink::of(head.slug, head.related),
            );
            Some((tier, found))
        })
        .collect()
}

fn group_rows<'a>(
    inputs: &SiteInputs<'a>,
    slugs: &[&Slug],
    row: impl Fn(&Slug, &'a Entry, Option<Tier>) -> EntryRow<'a>,
) -> Vec<EntryGroup<'a>> {
    let report = inputs.report;
    let mut rows = family_rows(inputs, slugs, |slug, entry, tier| {
        let record = report.and_then(|report| report.entries.get(slug));
        row(slug, entry, tier).with_choice(record)
    });
    if report.is_none() {
        let all: Vec<EntryRow<'a>> = rows.into_iter().map(|(_, row)| row).collect();
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
        let group: Vec<EntryRow<'a>> = taken.into_iter().map(|(_, row)| row).collect();
        if group.is_empty() {
            continue;
        }
        groups.push(EntryGroup {
            heading: Some(heading_tier.map_or(NOT_CHECKED, tier_name)),
            rows: group,
        });
    }
    groups
}
