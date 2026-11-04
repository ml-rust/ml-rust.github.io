//! The one order of every entry list, and the rows it collapses into families.
//!
//! This is an order and never a score. No page prints a rank or a composite.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use catalogue_core::{ACTIVE_USERS_SIGNAL, Catalogue, Slug, Tier, TierReport};

use super::entry_row::tier_of;

/// The sentence a list with tiers shows.
const STRONGEST_FIRST: &str =
    "Order: strongest tier first, then most active users, then name, with unmeasured values last.";

/// The sentence a list without tiers shows.
const BY_NAME: &str = "Entries are sorted by name.";

/// The sentence that states the order of a list built with `report`.
pub(crate) fn order_text(report: Option<&TierReport>) -> &'static str {
    if report.is_some() {
        STRONGEST_FIRST
    } else {
        BY_NAME
    }
}

/// What orders one entry.
struct Strength<'s> {
    /// The tier's place in [`Tier::ALL`]. An entry with no tier comes last.
    tier: usize,
    /// The active users. `None` when not measured.
    users: Option<f64>,
    /// The entry name in lowercase.
    name: String,
    slug: &'s Slug,
}

impl<'s> Strength<'s> {
    fn of(catalogue: &Catalogue, report: Option<&TierReport>, slug: &'s Slug) -> Self {
        let tier = tier_of(report, slug)
            .and_then(|tier| Tier::ALL.iter().position(|listed| *listed == tier))
            .unwrap_or(Tier::ALL.len());
        let users = report
            .and_then(|report| report.entries.get(slug))
            .and_then(|record| record.value_of(ACTIVE_USERS_SIGNAL))
            .and_then(|text| text.parse::<f64>().ok())
            .filter(|value| value.is_finite());
        let name = catalogue
            .entry(slug)
            .map_or_else(String::new, |entry| entry.name.to_lowercase());
        Self {
            tier,
            users,
            name,
            slug,
        }
    }

    fn order(&self, other: &Self) -> Ordering {
        self.tier
            .cmp(&other.tier)
            .then_with(|| most_users_first(self.users, other.users))
            .then_with(|| self.name.cmp(&other.name))
            .then_with(|| self.slug.cmp(other.slug))
    }
}

/// The larger count first. A value not measured comes after every measured one.
fn most_users_first(a: Option<f64>, b: Option<f64>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => b.total_cmp(&a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// `slugs` in the order of every list: tier, then active users from most to fewest, then name.
///
/// A value not measured sorts last. Without a report, the name alone decides.
pub(crate) fn strongest_first<'s>(
    catalogue: &Catalogue,
    report: Option<&TierReport>,
    slugs: impl IntoIterator<Item = &'s Slug>,
) -> Vec<&'s Slug> {
    let mut found: Vec<Strength<'s>> = slugs
        .into_iter()
        .map(|slug| Strength::of(catalogue, report, slug))
        .collect();
    found.sort_by(Strength::order);
    found.into_iter().map(|strength| strength.slug).collect()
}

/// One row of a list: an entry, and how many of its related projects the row stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FamilyHead<'s> {
    /// The entry the row shows.
    pub slug: &'s Slug,
    /// The entries of the list that name this one in `part_of`.
    pub related: usize,
}

/// The parent of `slug` when the parent is in `listed`.
fn parent_in_list<'c>(
    catalogue: &'c Catalogue,
    listed: &BTreeSet<&Slug>,
    slug: &Slug,
) -> Option<&'c Slug> {
    catalogue
        .entry(slug)
        .and_then(|entry| entry.part_of.as_ref())
        .filter(|parent| listed.contains(parent))
}

/// The rows of a list of `slugs`, in the order of [`strongest_first`].
///
/// An entry whose parent is in the list has no row, and its parent counts it. An entry whose
/// parent is not in the list has its own row.
pub(crate) fn family_heads<'s>(
    catalogue: &Catalogue,
    report: Option<&TierReport>,
    slugs: &[&'s Slug],
) -> Vec<FamilyHead<'s>> {
    let listed: BTreeSet<&Slug> = slugs.iter().copied().collect();
    let mut counts: BTreeMap<&Slug, usize> = BTreeMap::new();
    for &slug in &listed {
        if let Some(parent) = parent_in_list(catalogue, &listed, slug) {
            *counts.entry(parent).or_default() += 1;
        }
    }
    strongest_first(catalogue, report, slugs.iter().copied())
        .into_iter()
        .filter(|slug| parent_in_list(catalogue, &listed, slug).is_none())
        .map(|slug| FamilyHead {
            slug,
            related: counts.get(slug).copied().unwrap_or_default(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use catalogue_core::Tier;

    use super::*;
    use crate::testing::{catalogue, entry, record, report, slug};

    fn sorted(data: &Catalogue, tiers: Option<&TierReport>, slugs: &[Slug]) -> Vec<String> {
        strongest_first(data, tiers, slugs)
            .into_iter()
            .map(ToString::to_string)
            .collect()
    }

    fn all(data: &Catalogue) -> Vec<Slug> {
        data.entries().keys().cloned().collect()
    }

    #[test]
    fn tier_comes_first_then_users_then_name_with_unmeasured_last() {
        let data = catalogue(vec![
            entry("archived", ""),
            entry("few", ""),
            entry("many", ""),
            entry("unmeasured", ""),
            entry("not-a-number", ""),
            entry("tied-b", ""),
            entry("tied-a", ""),
            entry("top", ""),
        ]);
        let tiers = report(vec![
            ("archived", record(Tier::Archived, Some("9999"))),
            ("few", record(Tier::Listed, Some("2.5"))),
            ("many", record(Tier::Listed, Some("10"))),
            ("unmeasured", record(Tier::Listed, None)),
            ("not-a-number", record(Tier::Listed, Some("lots"))),
            ("tied-b", record(Tier::Listed, Some("1"))),
            ("tied-a", record(Tier::Listed, Some("1"))),
            ("top", record(Tier::Featured, None)),
        ]);
        assert_eq!(
            sorted(&data, Some(&tiers), &all(&data)),
            [
                "top",
                "many",
                "few",
                "tied-a",
                "tied-b",
                "not-a-number",
                "unmeasured",
                "archived",
            ]
        );
    }

    #[test]
    fn an_entry_with_no_record_sorts_after_every_tier() {
        let data = catalogue(vec![entry("alone", ""), entry("archived", "")]);
        let tiers = report(vec![("archived", record(Tier::Archived, None))]);
        assert_eq!(
            sorted(&data, Some(&tiers), &all(&data)),
            ["archived", "alone"]
        );
    }

    #[test]
    fn without_a_report_the_name_decides() {
        let data = catalogue(vec![entry("zeta", ""), entry("alpha", "")]);
        assert_eq!(sorted(&data, None, &all(&data)), ["alpha", "zeta"]);
    }

    #[test]
    fn the_order_sentence_names_the_order_and_no_score() {
        let tiers = report(Vec::new());
        assert!(order_text(Some(&tiers)).contains("most active users"));
        assert_eq!(order_text(None), BY_NAME);
    }

    fn heads(data: &Catalogue, slugs: &[Slug]) -> Vec<(String, usize)> {
        let refs: Vec<&Slug> = slugs.iter().collect();
        family_heads(data, None, &refs)
            .into_iter()
            .map(|head| (head.slug.to_string(), head.related))
            .collect()
    }

    #[test]
    fn a_parent_in_the_list_stands_for_its_children() {
        let data = catalogue(vec![
            entry("core", ""),
            entry("core-a", "part_of = \"core\"\n"),
            entry("core-b", "part_of = \"core\"\n"),
            entry("other", ""),
        ]);
        assert_eq!(
            heads(&data, &all(&data)),
            [("core".to_owned(), 2), ("other".to_owned(), 0)]
        );
    }

    #[test]
    fn a_child_without_its_parent_in_the_list_keeps_its_row() {
        let data = catalogue(vec![
            entry("core", ""),
            entry("core-a", "part_of = \"core\"\n"),
        ]);
        assert_eq!(heads(&data, &[slug("core-a")]), [("core-a".to_owned(), 0)]);
    }

    #[test]
    fn the_count_covers_only_the_children_in_the_list() {
        let data = catalogue(vec![
            entry("core", ""),
            entry("core-a", "part_of = \"core\"\n"),
            entry("core-b", "part_of = \"core\"\n"),
        ]);
        assert_eq!(
            heads(&data, &[slug("core"), slug("core-a")]),
            [("core".to_owned(), 1)]
        );
    }

    #[test]
    fn the_family_row_sorts_by_the_parent() {
        let data = catalogue(vec![
            entry("alpha", ""),
            entry("zeta", ""),
            entry("zeta-a", "part_of = \"zeta\"\n"),
        ]);
        let tiers = report(vec![
            ("alpha", record(Tier::Listed, Some("5"))),
            ("zeta", record(Tier::Listed, Some("1"))),
            ("zeta-a", record(Tier::Featured, Some("99"))),
        ]);
        let slugs = all(&data);
        let refs: Vec<&Slug> = slugs.iter().collect();
        let found: Vec<&str> = family_heads(&data, Some(&tiers), &refs)
            .into_iter()
            .map(|head| head.slug.as_str())
            .collect();
        assert_eq!(found, ["alpha", "zeta"]);
    }
}
