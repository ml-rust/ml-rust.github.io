//! The tier changes a history records.

use std::collections::BTreeMap;

use catalogue_core::{Date, Tier};

use super::query::History;

/// An entry's tier moving between two of its snapshot rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TierChange {
    /// The date of the snapshot that holds the new tier.
    pub date: Date,
    /// The entry slug.
    pub slug: String,
    /// The tier of the entry's row before. `None` on its first row.
    pub from: Option<Tier>,
    /// The new tier.
    pub to: Tier,
}

impl History {
    /// Every tier change across the snapshots, newest first. Changes on one date sort by slug.
    ///
    /// - The first row of a slug is a change with `from: None`.
    /// - A row whose tier differs from the slug's row before it is a change.
    /// - A slug missing from a snapshot is not a change. When the slug returns,
    ///   its row compares with its last row before the gap.
    pub fn tier_changes(&self) -> Vec<TierChange> {
        let mut last: BTreeMap<&str, Tier> = BTreeMap::new();
        let mut changes = Vec::new();
        for snapshot in self.snapshots() {
            for (slug, row) in &snapshot.rows {
                let from = last.insert(slug.as_str(), row.tier);
                if from != Some(row.tier) {
                    changes.push(TierChange {
                        date: snapshot.date,
                        slug: slug.clone(),
                        from,
                        to: row.tier,
                    });
                }
            }
        }
        changes.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.slug.cmp(&b.slug)));
        changes
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::EntryKind;

    use super::*;
    use crate::history::{Row, Snapshot};

    fn date(day: i8) -> Date {
        Date::new(2026, 10, day).expect("date")
    }

    fn snapshot(day: i8, rows: &[(&str, Tier)]) -> Snapshot {
        let row = |tier| Row {
            kind: EntryKind::Library,
            tier,
            dead_now: false,
            dormant: None,
            active_users: None,
            docker_pulls: None,
            network: None,
            reused: false,
            fetch_notes: Vec::new(),
        };
        Snapshot {
            date: date(day),
            rows: rows
                .iter()
                .map(|&(slug, tier)| (slug.to_owned(), row(tier)))
                .collect(),
        }
    }

    fn change(day: i8, slug: &str, from: Option<Tier>, to: Tier) -> TierChange {
        TierChange {
            date: date(day),
            slug: slug.to_owned(),
            from,
            to,
        }
    }

    #[test]
    fn changes_come_from_consecutive_rows_newest_first() {
        let history = History::new(vec![
            snapshot(3, &[("a", Tier::Featured), ("c", Tier::Archived)]),
            snapshot(1, &[("b", Tier::New), ("a", Tier::Listed)]),
            snapshot(
                2,
                &[("a", Tier::Featured), ("b", Tier::New), ("c", Tier::Listed)],
            ),
        ]);
        assert_eq!(
            history.tier_changes(),
            [
                change(3, "c", Some(Tier::Listed), Tier::Archived),
                change(2, "a", Some(Tier::Listed), Tier::Featured),
                change(2, "c", None, Tier::Listed),
                change(1, "a", None, Tier::Listed),
                change(1, "b", None, Tier::New),
            ]
        );
        assert!(History::default().tier_changes().is_empty());
    }

    #[test]
    fn a_returning_slug_compares_with_its_last_row() {
        let history = History::new(vec![
            snapshot(1, &[("a", Tier::Listed), ("b", Tier::New)]),
            snapshot(2, &[("b", Tier::New)]),
            snapshot(3, &[("a", Tier::Listed), ("b", Tier::Listed)]),
        ]);
        assert_eq!(
            history.tier_changes(),
            [
                change(3, "b", Some(Tier::New), Tier::Listed),
                change(1, "a", None, Tier::Listed),
                change(1, "b", None, Tier::New),
            ]
        );
    }
}
