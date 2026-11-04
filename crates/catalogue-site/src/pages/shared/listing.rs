//! Entry lists: one row per project family, in the one order of every list.

use catalogue_core::{Slug, Tier};

use super::badge::TierBadge;
use super::entry_row::{EntryRow, RelatedLink, tier_of};
use super::labels::tier_name;
use super::movement::movement_word;
use super::order::family_heads;
use crate::inputs::SiteInputs;

/// The heading of the group for entries that the report holds no record for.
const NOT_CHECKED: &str = "Not checked";

/// One group of entry rows, as `entry_table.html` reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryGroup<'a> {
    /// The group heading. `None` for the one list of a build without a report.
    pub heading: Option<&'static str>,
    /// The anchor of the group and the class of its tier. `None` without a tier.
    pub tier_class: Option<&'static str>,
    /// How many projects the rows stand for, as text.
    pub count: String,
    /// How many projects the rows stand for.
    pub projects: usize,
    /// The rows, in the one order of every list.
    pub rows: Vec<EntryRow<'a>>,
}

impl<'a> EntryGroup<'a> {
    /// The group of `rows`, under `heading` when given.
    fn new(
        heading: Option<&'static str>,
        tier_class: Option<&'static str>,
        rows: Vec<EntryRow<'a>>,
    ) -> Self {
        let projects = rows.iter().map(EntryRow::project_count).sum();
        Self {
            heading,
            tier_class,
            count: project_count(projects),
            projects,
            rows,
        }
    }

    /// One group with no heading, for a list that holds one tier.
    pub(crate) fn plain(rows: Vec<EntryRow<'a>>) -> Self {
        Self::new(None, None, rows)
    }
}

/// `count` projects as text: `1 project`, `3 projects`.
pub(crate) fn project_count(count: usize) -> String {
    if count == 1 {
        "1 project".to_owned()
    } else {
        format!("{count} projects")
    }
}

/// The entries `slugs`, grouped for display, each row with its facts and how to get it.
///
/// With a report: one group per tier, from Featured to Archived, with no empty
/// group. Without a report: one group with no heading.
pub(crate) fn group_entries<'a>(inputs: &SiteInputs<'a>, slugs: &[&Slug]) -> Vec<EntryGroup<'a>> {
    let mut rows = family_rows(inputs, slugs);
    if inputs.report.is_none() {
        let all: Vec<EntryRow<'a>> = rows.into_iter().map(|(_, row)| row).collect();
        return if all.is_empty() {
            Vec::new()
        } else {
            vec![EntryGroup::plain(all)]
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
        groups.push(EntryGroup::new(
            Some(heading_tier.map_or(NOT_CHECKED, tier_name)),
            heading_tier.map(|tier| TierBadge::of(tier).class),
            group,
        ));
    }
    groups
}

/// The slugs of every Featured entry.
pub(crate) fn featured_slugs<'a>(inputs: &SiteInputs<'a>) -> Vec<&'a Slug> {
    inputs
        .catalogue
        .entries()
        .keys()
        .filter(|slug| tier_of(inputs.report, slug) == Some(Tier::Featured))
        .collect()
}

/// One row per family of the Featured entries, in the one order of every list.
pub(crate) fn featured_rows<'a>(inputs: &SiteInputs<'a>) -> Vec<EntryRow<'a>> {
    family_rows(inputs, &featured_slugs(inputs))
        .into_iter()
        .map(|(_, row)| row)
        .collect()
}

/// One row per family of `slugs`, each with its tier.
fn family_rows<'a>(inputs: &SiteInputs<'a>, slugs: &[&Slug]) -> Vec<(Option<Tier>, EntryRow<'a>)> {
    let catalogue = inputs.catalogue;
    let report = inputs.report;
    family_heads(catalogue, report, slugs)
        .into_iter()
        .filter_map(|head| {
            let entry = catalogue.entry(head.slug)?;
            let tier = tier_of(report, head.slug);
            let record = report.and_then(|report| report.entries.get(head.slug));
            let found = EntryRow::new(head.slug, entry, tier)
                .with_choice(record)
                .with_family(
                    movement_word(inputs, head.slug),
                    RelatedLink::of(head.slug, head.related),
                );
            Some((tier, found))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_count_text_names_one_project_in_the_singular() {
        assert_eq!(project_count(0), "0 projects");
        assert_eq!(project_count(1), "1 project");
        assert_eq!(project_count(3), "3 projects");
    }
}
