//! The movement a history records: new entries and changes in active users.
//!
//! No tier rule reads it.

use std::collections::BTreeMap;

use catalogue_core::{
    Date, FirstSeen, MeasuredMovement, Movement, MovementThresholds, Slug, UsersChange,
};

use super::model::{Row, Snapshot};
use super::query::History;

impl History {
    /// The date of the oldest snapshot. Every entry of that snapshot appears on it.
    pub fn first_check(&self) -> Option<Date> {
        self.snapshots().first().map(|snapshot| snapshot.date)
    }

    /// The movement over the window of `limits`, measured back from the newest snapshot.
    ///
    /// The result is [`Movement::NeedsHistory`] when the history spans less than the
    /// window, or when the snapshot nearest the window start is the newest one.
    ///
    /// - New: the slug's oldest snapshot lies inside the window, and it is not the
    ///   oldest snapshot of the history.
    /// - Rising and falling: active users changed by at least `min_users` and by at
    ///   least `min_share` of the baseline value, where the baseline is the snapshot
    ///   nearest the window start. The older snapshot wins a tie. A value missing from
    ///   either snapshot gives no change.
    pub fn movement(&self, limits: &MovementThresholds) -> Movement {
        let snapshots = self.snapshots();
        let (Some(oldest), Some(newest)) = (snapshots.first(), snapshots.last()) else {
            return Movement::NeedsHistory;
        };
        let window = i64::from(limits.window_days);
        if newest.date.days_since(oldest.date) < window {
            return Movement::NeedsHistory;
        }
        let Some(baseline) = snapshots
            .iter()
            .min_by_key(|snapshot| newest.date.days_since(snapshot.date).abs_diff(window))
        else {
            return Movement::NeedsHistory;
        };
        if baseline.date == newest.date {
            return Movement::NeedsHistory;
        }
        let (rising, falling) = changes(baseline, newest, limits);
        Movement::Measured(MeasuredMovement {
            newest: newest.date,
            baseline: baseline.date,
            new: first_seen_inside(snapshots, oldest.date, newest.date, window),
            rising,
            falling,
        })
    }
}

/// The slugs whose oldest snapshot lies inside the window and is not the first day of history.
fn first_seen_inside(
    snapshots: &[Snapshot],
    first_day: Date,
    newest: Date,
    window: i64,
) -> Vec<FirstSeen> {
    let mut seen: BTreeMap<&str, Date> = BTreeMap::new();
    for snapshot in snapshots {
        for slug in snapshot.rows.keys() {
            seen.entry(slug).or_insert(snapshot.date);
        }
    }
    seen.into_iter()
        .filter(|&(_, date)| date != first_day && newest.days_since(date) <= window)
        .filter_map(|(slug, date)| {
            Some(FirstSeen {
                slug: Slug::new(slug).ok()?,
                date,
            })
        })
        .collect()
}

/// The slugs that rose and the slugs that fell between `baseline` and `newest`.
fn changes(
    baseline: &Snapshot,
    newest: &Snapshot,
    limits: &MovementThresholds,
) -> (Vec<UsersChange>, Vec<UsersChange>) {
    let mut rising = Vec::new();
    let mut falling = Vec::new();
    for (name, row) in &newest.rows {
        let Some(from) = baseline.rows.get(name).and_then(users) else {
            continue;
        };
        let Some(to) = users(row) else {
            continue;
        };
        let Ok(slug) = Slug::new(name.as_str()) else {
            continue;
        };
        let change = UsersChange { slug, from, to };
        if counts(to - from, from, limits) {
            rising.push(change);
        } else if counts(from - to, from, limits) {
            falling.push(change);
        }
    }
    (rising, falling)
}

/// The active users of `row`, when measured.
fn users(row: &Row) -> Option<f64> {
    row.active_users.filter(|value| value.is_finite())
}

/// Whether a rise or fall of `amount` from `from` reaches both limits.
///
/// A baseline of zero makes any rise an unlimited share, so only `min_users` decides.
fn counts(amount: f64, from: f64, limits: &MovementThresholds) -> bool {
    amount >= limits.min_users && amount >= limits.min_share * from
}

#[cfg(test)]
mod tests {
    use catalogue_core::{EntryKind, Tier};
    use jiff::Span;

    use super::*;

    fn newest_day() -> Date {
        Date::new(2026, 11, 4).expect("date")
    }

    /// The date `days` before the newest snapshot.
    fn ago(days: i64) -> Date {
        let before = newest_day().civil().checked_sub(Span::new().days(days));
        Date::from(before.expect("date"))
    }

    fn row(users: Option<f64>) -> Row {
        Row {
            kind: EntryKind::Library,
            tier: Tier::Listed,
            dead_now: false,
            dormant: None,
            active_users: users,
            docker_pulls: None,
            network: None,
            reused: false,
            fetch_notes: Vec::new(),
        }
    }

    fn snapshot(days: i64, rows: &[(&str, Option<f64>)]) -> Snapshot {
        Snapshot {
            date: ago(days),
            rows: rows
                .iter()
                .map(|&(slug, users)| (slug.to_owned(), row(users)))
                .collect(),
        }
    }

    fn limits() -> MovementThresholds {
        MovementThresholds {
            window_days: 30,
            min_share: 0.25,
            min_users: 5.0,
        }
    }

    fn measured(snapshots: Vec<Snapshot>) -> MeasuredMovement {
        match History::new(snapshots).movement(&limits()) {
            Movement::Measured(found) => found,
            Movement::NeedsHistory => panic!("expected a measured window"),
        }
    }

    fn slugs(changes: &[UsersChange]) -> Vec<&str> {
        changes.iter().map(|change| change.slug.as_str()).collect()
    }

    fn new_slugs(found: &MeasuredMovement) -> Vec<&str> {
        found.new.iter().map(|item| item.slug.as_str()).collect()
    }

    #[test]
    fn an_empty_history_has_no_first_check_and_needs_history() {
        let history = History::default();
        assert_eq!(history.first_check(), None);
        assert_eq!(history.movement(&limits()), Movement::NeedsHistory);
    }

    #[test]
    fn one_snapshot_needs_history() {
        let history = History::new(vec![snapshot(0, &[("a", Some(1.0))])]);
        assert_eq!(history.first_check(), Some(ago(0)));
        assert_eq!(history.movement(&limits()), Movement::NeedsHistory);
    }

    #[test]
    fn a_history_shorter_than_the_window_gives_nothing_at_all() {
        let history = History::new(vec![
            snapshot(29, &[("a", Some(10.0))]),
            snapshot(1, &[("a", Some(10.0)), ("b", Some(1.0))]),
            snapshot(0, &[("a", Some(90.0)), ("b", Some(1.0))]),
        ]);
        assert_eq!(history.movement(&limits()), Movement::NeedsHistory);
    }

    #[test]
    fn a_history_of_exactly_the_window_measures() {
        let found = measured(vec![
            snapshot(30, &[("a", Some(10.0))]),
            snapshot(0, &[("a", Some(20.0))]),
        ]);
        assert_eq!(found.newest, ago(0));
        assert_eq!(found.baseline, ago(30));
        assert_eq!(slugs(&found.rising), ["a"]);
    }

    #[test]
    fn the_baseline_is_the_snapshot_nearest_the_window_start() {
        let history = |near: i64| {
            measured(vec![
                snapshot(50, &[("a", Some(1.0))]),
                snapshot(near, &[("a", Some(1.0))]),
                snapshot(0, &[("a", Some(1.0))]),
            ])
        };
        assert_eq!(history(28).baseline, ago(28));
        assert_eq!(history(33).baseline, ago(33));
        let tie = measured(vec![
            snapshot(32, &[("a", Some(1.0))]),
            snapshot(28, &[("a", Some(1.0))]),
            snapshot(0, &[("a", Some(1.0))]),
        ]);
        assert_eq!(tie.baseline, ago(32), "the older snapshot wins a tie");
    }

    #[test]
    fn a_baseline_that_is_the_newest_snapshot_needs_history() {
        let history = History::new(vec![
            snapshot(100, &[("a", Some(1.0))]),
            snapshot(0, &[("a", Some(90.0))]),
        ]);
        assert_eq!(history.movement(&limits()), Movement::NeedsHistory);
    }

    #[test]
    fn both_limits_must_hold_in_either_direction() {
        let found = measured(vec![
            snapshot(
                30,
                &[
                    ("rises", Some(100.0)),
                    ("share-only", Some(4.0)),
                    ("users-only", Some(100.0)),
                    ("falls", Some(100.0)),
                    ("falls-share-only", Some(4.0)),
                    ("falls-users-only", Some(100.0)),
                    ("still", Some(100.0)),
                ],
            ),
            snapshot(
                0,
                &[
                    ("rises", Some(125.0)),
                    ("share-only", Some(8.0)),
                    ("users-only", Some(124.0)),
                    ("falls", Some(75.0)),
                    ("falls-share-only", Some(1.0)),
                    ("falls-users-only", Some(76.0)),
                    ("still", Some(100.0)),
                ],
            ),
        ]);
        assert_eq!(slugs(&found.rising), ["rises"]);
        assert_eq!(slugs(&found.falling), ["falls"]);
        assert_eq!(found.rising[0].from, 100.0);
        assert_eq!(found.rising[0].to, 125.0);
    }

    #[test]
    fn a_rise_from_zero_needs_only_the_user_count() {
        let found = measured(vec![
            snapshot(30, &[("up", Some(0.0)), ("small", Some(0.0))]),
            snapshot(0, &[("up", Some(5.0)), ("small", Some(4.0))]),
        ]);
        assert_eq!(slugs(&found.rising), ["up"]);
    }

    #[test]
    fn a_value_missing_at_either_end_gives_no_change() {
        let found = measured(vec![
            snapshot(
                30,
                &[
                    ("old-unknown", None),
                    ("new-unknown", Some(100.0)),
                    ("both-unknown", None),
                    ("gone", Some(100.0)),
                    ("not-finite", Some(f64::NAN)),
                ],
            ),
            snapshot(
                0,
                &[
                    ("old-unknown", Some(100.0)),
                    ("new-unknown", None),
                    ("both-unknown", None),
                    ("not-finite", Some(100.0)),
                ],
            ),
        ]);
        assert!(found.rising.is_empty(), "{:?}", found.rising);
        assert!(found.falling.is_empty(), "{:?}", found.falling);
    }

    #[test]
    fn a_slug_absent_from_the_baseline_gives_no_change() {
        let found = measured(vec![
            snapshot(30, &[("a", Some(1.0))]),
            snapshot(10, &[("a", Some(1.0)), ("b", Some(1.0))]),
            snapshot(0, &[("a", Some(1.0)), ("b", Some(900.0))]),
        ]);
        assert!(found.rising.is_empty());
        assert_eq!(new_slugs(&found), ["b"]);
    }

    #[test]
    fn the_first_day_of_history_is_never_new() {
        let found = measured(vec![
            snapshot(30, &[("first-day", Some(1.0))]),
            snapshot(0, &[("first-day", Some(1.0))]),
        ]);
        assert!(found.new.is_empty(), "{:?}", found.new);
    }

    #[test]
    fn new_means_first_seen_inside_the_window() {
        let found = measured(vec![
            snapshot(60, &[("old", None)]),
            snapshot(31, &[("old", None), ("before", None)]),
            snapshot(30, &[("old", None), ("before", None), ("edge", None)]),
            snapshot(5, &[("inside", None), ("left-again", None)]),
            snapshot(0, &[("old", None), ("before", None), ("inside", None)]),
        ]);
        assert_eq!(new_slugs(&found), ["edge", "inside", "left-again"]);
        assert_eq!(found.new[1].date, ago(5));
    }

    #[test]
    fn a_returning_slug_keeps_its_first_date() {
        let found = measured(vec![
            snapshot(60, &[("back", None), ("a", None)]),
            snapshot(30, &[("a", None)]),
            snapshot(0, &[("back", None), ("a", None)]),
        ]);
        assert!(found.new.is_empty(), "{:?}", found.new);
    }
}
