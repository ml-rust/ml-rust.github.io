//! One tier change as a row: date, project link, old tier and new tier.

use std::cmp::Reverse;

use catalogue_core::{Catalogue, Date, Tier};

use super::badge::TierBadge;
use super::date_text::date_text;
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
    /// The badge of the old tier. `None` for an entry's first check.
    pub from_badge: Option<TierBadge>,
    /// The badge of the new tier.
    pub to_badge: TierBadge,
}

/// The changes of one check, as one stop of the timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangeDay<'a> {
    /// The date of the check.
    pub date: String,
    /// The date as a person reads it.
    pub date_text: String,
    /// The changes on the date, in input order.
    pub rows: Vec<ChangeRow<'a>>,
}

/// `rows`, newest first, gathered by date.
pub(crate) fn change_days<'a>(rows: Vec<ChangeRow<'a>>) -> Vec<ChangeDay<'a>> {
    let mut days: Vec<ChangeDay<'a>> = Vec::new();
    for row in rows {
        match days.last_mut() {
            Some(day) if day.date == row.date => day.rows.push(row),
            _ => days.push(ChangeDay {
                date_text: date_text(&row.date).unwrap_or_else(|| row.date.clone()),
                date: row.date.clone(),
                rows: vec![row],
            }),
        }
    }
    days
}

/// The one line that stands for every row of the first check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FirstCheckLine {
    /// The date of the first check.
    pub date: String,
    /// The date as a person reads it.
    pub date_text: String,
    /// The entry count and the count per tier, such as `3 entries: Featured 1, Listed 2`.
    pub text: String,
    /// The entry count as words, such as `3 entries`.
    pub total: String,
    /// The count per tier, strongest first, with no empty tier.
    pub tiers: Vec<TierCount>,
}

/// How many entries the first check placed in one tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TierCount {
    /// The tier badge.
    pub badge: TierBadge,
    /// The entry count.
    pub count: usize,
}

/// Whether `change` is a row of the first check of the history.
///
/// Every entry appears on that date, so its rows say nothing about one entry.
pub(crate) fn is_first_check(change: &TierChangeRow, first_check: Option<Date>) -> bool {
    change.from.is_none() && Some(change.date) == first_check
}

/// The line for the first check. `None` when `changes` holds no row of it.
pub(crate) fn first_check_line(
    changes: &[TierChangeRow],
    first_check: Option<Date>,
) -> Option<FirstCheckLine> {
    let date = first_check?;
    let rows: Vec<&TierChangeRow> = changes
        .iter()
        .filter(|change| is_first_check(change, first_check))
        .collect();
    let total = match rows.len() {
        0 => return None,
        1 => "1 entry".to_owned(),
        more => format!("{more} entries"),
    };
    let tiers: Vec<TierCount> = Tier::ALL
        .into_iter()
        .filter_map(|tier| {
            let count = rows.iter().filter(|change| change.to == tier).count();
            (count > 0).then(|| TierCount {
                badge: TierBadge::of(tier),
                count,
            })
        })
        .collect();
    let counts: Vec<String> = tiers
        .iter()
        .map(|tier| format!("{} {}", tier.badge.name, tier.count))
        .collect();
    let date = date.to_string();
    Some(FirstCheckLine {
        date_text: date_text(&date).unwrap_or_else(|| date.clone()),
        date,
        text: format!("{total}: {}", counts.join(", ")),
        total,
        tiers,
    })
}

/// The rows of `changes`, newest first, at most `limit` of them.
///
/// Changes on the same date keep their input order. The rows of the first check stay out:
/// [`first_check_line`] stands for them.
pub(crate) fn change_rows<'a>(
    catalogue: &'a Catalogue,
    changes: &'a [TierChangeRow],
    first_check: Option<Date>,
    limit: usize,
) -> Vec<ChangeRow<'a>> {
    let mut sorted: Vec<&TierChangeRow> = changes
        .iter()
        .filter(|change| !is_first_check(change, first_check))
        .collect();
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
                from_badge: change.from.map(TierBadge::of),
                to_badge: TierBadge::of(change.to),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use catalogue_core::Slug;

    use super::*;

    fn day(on: i8) -> Date {
        Date::new(2026, 10, on).expect("date")
    }

    fn change(on: i8, slug: &str, from: Option<Tier>, to: Tier) -> TierChangeRow {
        TierChangeRow {
            date: day(on),
            slug: Slug::new(slug).expect("slug"),
            from,
            to,
        }
    }

    #[test]
    fn the_line_counts_the_first_check_and_skips_an_empty_tier() {
        let changes = [
            change(5, "later", None, Tier::New),
            change(1, "a", None, Tier::Listed),
            change(1, "b", None, Tier::Featured),
            change(1, "c", None, Tier::Listed),
        ];
        let line = first_check_line(&changes, Some(day(1))).expect("line");
        assert_eq!(line.date, "2026-10-01");
        assert_eq!(line.text, "3 entries: Featured 1, Listed 2");
    }

    #[test]
    fn one_entry_reads_in_the_singular() {
        let changes = [change(1, "a", None, Tier::Listed)];
        let line = first_check_line(&changes, Some(day(1))).expect("line");
        assert_eq!(line.text, "1 entry: Listed 1");
    }

    #[test]
    fn no_history_or_no_row_gives_no_line() {
        let changes = [change(5, "a", None, Tier::Listed)];
        assert_eq!(first_check_line(&changes, None), None);
        assert_eq!(first_check_line(&changes, Some(day(1))), None);
    }

    #[test]
    fn only_the_first_check_loses_its_rows() {
        assert!(is_first_check(
            &change(1, "a", None, Tier::Listed),
            Some(day(1))
        ));
        assert!(!is_first_check(
            &change(5, "a", None, Tier::Listed),
            Some(day(1))
        ));
        assert!(!is_first_check(
            &change(1, "a", Some(Tier::New), Tier::Listed),
            Some(day(1))
        ));
        assert!(!is_first_check(&change(1, "a", None, Tier::Listed), None));
    }
}
