//! The questions the tier rules ask about earlier checks.

use std::collections::BTreeMap;

use catalogue_core::{Date, Tier};

use super::model::{Row, Snapshot};
use super::network::NetworkValues;

/// The fewest days between the Docker baseline snapshot and today.
///
/// Growth compares today's pull count with a count at least this old.
pub const GROWTH_WINDOW_DAYS: i64 = 90;

/// The most days between the Docker baseline snapshot and today.
///
/// An older snapshot no longer measures the growth window, so it is no baseline.
pub const GROWTH_MAX_AGE_DAYS: i64 = 120;

/// Every snapshot of a history directory, oldest first.
///
/// Each query takes `today` and reads only snapshots dated before it. A run
/// repeated on the same day therefore never sees its own earlier snapshot.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct History {
    snapshots: Vec<Snapshot>,
}

impl History {
    /// A history of `snapshots`, in any order.
    pub fn new(mut snapshots: Vec<Snapshot>) -> Self {
        snapshots.sort_by_key(|snapshot| snapshot.date);
        Self { snapshots }
    }

    /// Every snapshot, oldest first.
    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snapshots
    }

    /// Whether the history holds no snapshot.
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// The snapshots dated before `today`, newest first.
    fn earlier(&self, today: Date) -> impl Iterator<Item = &Snapshot> {
        self.snapshots
            .iter()
            .rev()
            .filter(move |snapshot| snapshot.date < today)
    }

    /// The tier of `slug` in the newest snapshot before `today` that holds it.
    pub fn previous_tier(&self, slug: &str, today: Date) -> Option<Tier> {
        self.previous_row(slug, today).map(|row| row.tier)
    }

    /// The network values of `slug` in the newest snapshot before `today` that holds it.
    ///
    /// Only that snapshot counts. An older one holds a value the newest check
    /// already replaced or dropped.
    pub fn previous_network(&self, slug: &str, today: Date) -> Option<&NetworkValues> {
        self.previous_row(slug, today)?.network.as_ref()
    }

    /// The row of `slug` in the newest snapshot before `today` that holds it.
    fn previous_row(&self, slug: &str, today: Date) -> Option<&Row> {
        self.earlier(today)
            .find_map(|snapshot| snapshot.rows.get(slug))
    }

    /// The count of consecutive calendar months, before the month of `today`,
    /// whose last snapshot finds `slug` dead.
    ///
    /// Each month is represented by its last snapshot. The count starts at the
    /// calendar month right before the month of `today` and walks back one
    /// month at a time. It stops at the first month whose row is not dead, has
    /// no row for `slug`, or has no snapshot at all. A history that ends before
    /// last month counts 0.
    pub fn prior_dead_checks(&self, slug: &str, today: Date) -> u32 {
        let current = month_index(today);
        let mut last_of_month: BTreeMap<i32, &Snapshot> = BTreeMap::new();
        for snapshot in &self.snapshots {
            let month = month_index(snapshot.date);
            if month < current {
                last_of_month.insert(month, snapshot);
            }
        }
        let mut count: u32 = 0;
        let mut expected = current.saturating_sub(1);
        for (&month, snapshot) in last_of_month.iter().rev() {
            if month != expected {
                break;
            }
            if !snapshot.rows.get(slug).is_some_and(|row| row.dead_now) {
                break;
            }
            count = count.saturating_add(1);
            expected = month.saturating_sub(1);
        }
        count
    }

    /// The Docker pull count that growth measures against.
    ///
    /// The newest snapshot dated [`GROWTH_WINDOW_DAYS`] to [`GROWTH_MAX_AGE_DAYS`]
    /// days before `today`, inclusive, whose row for `slug` holds a pull count.
    pub fn docker_baseline(&self, slug: &str, today: Date) -> Option<(Date, u64)> {
        self.earlier(today)
            .map(|snapshot| (snapshot, today.days_since(snapshot.date)))
            .skip_while(|&(_, age)| age < GROWTH_WINDOW_DAYS)
            .take_while(|&(_, age)| age <= GROWTH_MAX_AGE_DAYS)
            .find_map(|(snapshot, _)| {
                let pulls = snapshot.rows.get(slug).and_then(|row| row.docker_pulls)?;
                Some((snapshot.date, pulls))
            })
    }
}

/// Months since year 0, so that consecutive months differ by one.
fn month_index(date: Date) -> i32 {
    let civil = date.civil();
    i32::from(civil.year())
        .saturating_mul(12)
        .saturating_add(i32::from(civil.month()))
}

#[cfg(test)]
mod tests {
    use catalogue_core::EntryKind;
    use jiff::Span;

    use super::*;
    use crate::history::Dated;

    fn date(year: i16, month: i8, day: i8) -> Date {
        Date::new(year, month, day).expect("date")
    }

    fn row(tier: Tier, dead_now: bool, docker_pulls: Option<u64>) -> Row {
        Row {
            kind: EntryKind::App,
            tier,
            dead_now,
            dormant: None,
            active_users: None,
            docker_pulls,
            network: None,
        }
    }

    fn snapshot(on: Date, rows: &[(&str, Row)]) -> Snapshot {
        Snapshot {
            date: on,
            rows: rows
                .iter()
                .map(|(slug, row)| ((*slug).to_owned(), row.clone()))
                .collect(),
        }
    }

    fn tiered(on: Date, slug: &str, tier: Tier) -> Snapshot {
        snapshot(on, &[(slug, row(tier, false, None))])
    }

    fn dead(on: Date) -> Snapshot {
        snapshot(on, &[("a", row(Tier::Listed, true, None))])
    }

    fn alive(on: Date) -> Snapshot {
        snapshot(on, &[("a", row(Tier::Listed, false, None))])
    }

    fn today() -> Date {
        date(2026, 11, 3)
    }

    #[test]
    fn previous_tier_reads_the_newest_earlier_snapshot_that_holds_the_slug() {
        let history = History::new(vec![
            tiered(date(2026, 10, 3), "a", Tier::Featured),
            tiered(date(2026, 10, 20), "a", Tier::Listed),
            tiered(date(2026, 10, 25), "b", Tier::New),
            tiered(date(2026, 11, 3), "a", Tier::Archived),
            tiered(date(2026, 11, 4), "a", Tier::Archived),
        ]);
        assert_eq!(history.previous_tier("a", today()), Some(Tier::Listed));
        assert_eq!(history.previous_tier("b", today()), Some(Tier::New));
        assert_eq!(history.previous_tier("ghost", today()), None);
        assert_eq!(History::default().previous_tier("a", today()), None);
    }

    #[test]
    fn dead_months_count_back_from_the_latest_month_before_today() {
        let history = History::new(vec![
            dead(date(2026, 8, 15)),
            dead(date(2026, 9, 15)),
            dead(date(2026, 10, 15)),
        ]);
        assert_eq!(history.prior_dead_checks("a", today()), 3);
        assert_eq!(history.prior_dead_checks("a", date(2026, 10, 31)), 2);
        assert_eq!(History::default().prior_dead_checks("a", today()), 0);
    }

    #[test]
    fn a_month_without_a_snapshot_breaks_the_run() {
        let history = History::new(vec![
            dead(date(2026, 7, 15)),
            dead(date(2026, 9, 15)),
            dead(date(2026, 10, 15)),
        ]);
        assert_eq!(history.prior_dead_checks("a", today()), 2);
        let last_month_missing =
            History::new(vec![dead(date(2026, 7, 15)), dead(date(2026, 9, 15))]);
        assert_eq!(last_month_missing.prior_dead_checks("a", today()), 0);
        let stale = History::new(vec![dead(date(2026, 4, 15)), dead(date(2026, 5, 15))]);
        assert_eq!(stale.prior_dead_checks("a", today()), 0);
        let across_years = History::new(vec![dead(date(2025, 12, 31)), dead(date(2026, 1, 1))]);
        assert_eq!(across_years.prior_dead_checks("a", date(2026, 2, 1)), 2);
    }

    #[test]
    fn a_month_that_is_not_dead_breaks_the_run() {
        let history = History::new(vec![
            dead(date(2026, 8, 15)),
            alive(date(2026, 9, 15)),
            dead(date(2026, 10, 15)),
        ]);
        assert_eq!(history.prior_dead_checks("a", today()), 1);
        let latest_alive = History::new(vec![dead(date(2026, 9, 15)), alive(date(2026, 10, 15))]);
        assert_eq!(latest_alive.prior_dead_checks("a", today()), 0);
    }

    #[test]
    fn a_month_without_the_slug_breaks_the_run() {
        let history = History::new(vec![
            dead(date(2026, 8, 15)),
            snapshot(date(2026, 9, 15), &[("b", row(Tier::Listed, true, None))]),
            dead(date(2026, 10, 15)),
        ]);
        assert_eq!(history.prior_dead_checks("a", today()), 1);
        assert_eq!(history.prior_dead_checks("ghost", today()), 0);
    }

    #[test]
    fn the_last_snapshot_of_a_month_wins() {
        let history = History::new(vec![
            alive(date(2026, 10, 2)),
            dead(date(2026, 10, 20)),
            dead(date(2026, 9, 2)),
            alive(date(2026, 9, 20)),
        ]);
        assert_eq!(history.prior_dead_checks("a", today()), 1);
    }

    #[test]
    fn the_current_month_is_excluded() {
        let history = History::new(vec![dead(date(2026, 10, 15)), dead(date(2026, 11, 1))]);
        assert_eq!(history.prior_dead_checks("a", today()), 1);
        let only_current = History::new(vec![dead(date(2026, 11, 1)), dead(date(2026, 11, 2))]);
        assert_eq!(only_current.prior_dead_checks("a", today()), 0);
    }

    fn pulls(on: Date, count: Option<u64>) -> Snapshot {
        snapshot(on, &[("a", row(Tier::Listed, false, count))])
    }

    fn with_homebrew(on: Date, count: Option<u64>) -> Snapshot {
        let network = count.map(|value| NetworkValues {
            homebrew: Some(Dated {
                date: on,
                of: "tool".to_owned(),
                value,
            }),
            ..NetworkValues::default()
        });
        let row = Row {
            network,
            ..row(Tier::Listed, false, None)
        };
        snapshot(on, &[("a", row)])
    }

    fn previous_homebrew(history: &History, on: Date) -> Option<u64> {
        let network = history.previous_network("a", on)?;
        network.homebrew.as_ref().map(|found| found.value)
    }

    #[test]
    fn network_values_come_from_the_newest_earlier_snapshot_only() {
        let history = History::new(vec![
            with_homebrew(date(2026, 10, 30), Some(1)),
            with_homebrew(date(2026, 11, 1), Some(2)),
            with_homebrew(date(2026, 11, 3), Some(3)),
        ]);
        assert_eq!(previous_homebrew(&history, today()), Some(2));
        let dropped = History::new(vec![
            with_homebrew(date(2026, 10, 30), Some(1)),
            with_homebrew(date(2026, 11, 1), None),
        ]);
        assert_eq!(previous_homebrew(&dropped, today()), None);
        assert_eq!(History::default().previous_network("a", today()), None);
    }

    #[test]
    fn the_baseline_window_includes_90_and_120_days() {
        let today = today();
        let edge = |days: i64| {
            let before = today.civil().checked_sub(Span::new().days(days));
            let on = Date::from(before.expect("date"));
            History::new(vec![pulls(on, Some(7))]).docker_baseline("a", today)
        };
        assert_eq!(edge(89), None);
        assert_eq!(edge(90).map(|(_, count)| count), Some(7));
        assert_eq!(edge(120).map(|(_, count)| count), Some(7));
        assert_eq!(edge(121), None);
    }

    #[test]
    fn the_baseline_is_the_newest_snapshot_with_a_pull_count() {
        let history = History::new(vec![
            pulls(date(2026, 7, 10), Some(100)),
            pulls(date(2026, 7, 20), Some(200)),
            pulls(date(2026, 7, 25), None),
            pulls(date(2026, 10, 20), Some(900)),
        ]);
        assert_eq!(
            history.docker_baseline("a", today()),
            Some((date(2026, 7, 20), 200))
        );
        assert_eq!(history.docker_baseline("ghost", today()), None);
        assert_eq!(History::default().docker_baseline("a", today()), None);
    }
}
