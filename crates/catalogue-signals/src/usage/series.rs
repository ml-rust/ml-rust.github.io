//! The monthly user history of a target and the signals derived from it.

use std::collections::BTreeMap;

use jiff::civil;

use super::date::{add_days, next_month};
use super::runs::Run;
use super::trend::{former_glory, growth, peak};

/// Days before the evaluation date in which a run start counts as a new adoption.
pub(super) const NEW_ADOPTER_DAYS: i64 = 365;

/// The adoption signals of one target crate on the evaluation date.
#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    /// Weighted external users on the evaluation date.
    pub current: f64,
    /// The highest mean of two adjacent history samples.
    pub peak: f64,
    /// `current` against `peak`, from 0 to 1. 1 means no decline.
    pub former_glory: f64,
    /// The last two samples against the two samples four months before them.
    pub growth: f64,
    /// Weighted users on the evaluation date from runs that start in the last 365 days.
    pub new_adopters: f64,
    /// Ended runs whose depender released a version without the target.
    pub removed: u32,
    /// Ended runs that ran out at the end of the depender's latest-release lifetime.
    pub expired: u32,
    /// Weighted users at each month end, then on the evaluation date.
    pub history: Vec<(civil::Date, f64)>,
}

impl Usage {
    /// The target has fewer users now than at its peak.
    pub fn lost_users(&self) -> bool {
        self.current < self.peak
    }
}

/// The signals of one target from its runs.
///
/// `weigh` turns depender crate ids, ascending and distinct, into a user count.
pub(super) fn usage(runs: &[Run], today: civil::Date, weigh: &dyn Fn(&[u32]) -> f64) -> Usage {
    let history = history(runs, today, weigh);
    let current = weigh(&active(runs, today, None));
    let cutoff = add_days(today, -NEW_ADOPTER_DAYS);
    let new_adopters = weigh(&active(runs, today, Some(cutoff)));
    let peak = peak(&history);
    let (removed, expired) = ended(runs, today);
    Usage {
        current,
        peak,
        former_glory: former_glory(current, peak),
        growth: growth(&history, current),
        new_adopters,
        removed,
        expired,
        history,
    }
}

/// The last day of each month from the month of `first` to the month before
/// `today`, then `today`. Empty when `first` falls after `today`.
fn sample_dates(first: civil::Date, today: civil::Date) -> Vec<civil::Date> {
    let mut out = Vec::new();
    let last_month = today.first_of_month();
    let mut month = first.first_of_month();
    while month <= last_month {
        if month == last_month {
            out.push(today);
            break;
        }
        out.push(month.last_of_month());
        let Some(next) = next_month(month) else {
            break;
        };
        month = next;
    }
    out
}

/// Weighted users at each sample date, from one sweep over sorted run starts and ends.
fn history(
    runs: &[Run],
    today: civil::Date,
    weigh: &dyn Fn(&[u32]) -> f64,
) -> Vec<(civil::Date, f64)> {
    let Some(first) = runs.iter().map(|run| run.start).min() else {
        return Vec::new();
    };
    let mut starts: Vec<(civil::Date, u32)> =
        runs.iter().map(|run| (run.start, run.depender)).collect();
    let mut ends: Vec<(civil::Date, u32)> =
        runs.iter().map(|run| (run.end, run.depender)).collect();
    starts.sort_unstable();
    ends.sort_unstable();
    let mut starts = starts.into_iter().peekable();
    let mut ends = ends.into_iter().peekable();
    let mut open: BTreeMap<u32, u32> = BTreeMap::new();
    let mut out = Vec::new();
    let mut ids: Vec<u32> = Vec::new();
    for date in sample_dates(first, today) {
        while let Some((_, depender)) = starts.next_if(|&(start, _)| start <= date) {
            *open.entry(depender).or_insert(0) += 1;
        }
        while let Some((_, depender)) = ends.next_if(|&(end, _)| end <= date) {
            if let Some(count) = open.get_mut(&depender) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    open.remove(&depender);
                }
            }
        }
        ids.clear();
        ids.extend(open.keys().copied());
        out.push((date, weigh(&ids)));
    }
    out
}

/// The distinct dependers, ascending, with a run active on `date`.
///
/// With `after`, only runs that start after it count.
fn active(runs: &[Run], date: civil::Date, after: Option<civil::Date>) -> Vec<u32> {
    let mut ids: Vec<u32> = runs
        .iter()
        .filter(|run| after.is_none_or(|after| run.start > after))
        .filter(|run| run.start <= date && date < run.end)
        .map(|run| run.depender)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Runs that end on or before `today`: those a later release removed, then those that expired.
fn ended(runs: &[Run], today: civil::Date) -> (u32, u32) {
    runs.iter()
        .filter(|run| run.end <= today)
        .fold((0u32, 0u32), |(removed, expired), run| {
            if run.expired {
                (removed, expired.saturating_add(1))
            } else {
                (removed.saturating_add(1), expired)
            }
        })
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    fn run(depender: u32, start: civil::Date, end: civil::Date, expired: bool) -> Run {
        Run {
            depender,
            start,
            end,
            expired,
        }
    }

    fn count(ids: &[u32]) -> f64 {
        ids.len() as f64
    }

    #[test]
    fn samples_are_month_ends_then_today() {
        assert_eq!(
            sample_dates(date(2025, 11, 20), date(2026, 2, 10)),
            [
                date(2025, 11, 30),
                date(2025, 12, 31),
                date(2026, 1, 31),
                date(2026, 2, 10),
            ]
        );
        assert_eq!(
            sample_dates(date(2026, 2, 3), date(2026, 2, 10)),
            [date(2026, 2, 10)]
        );
        assert!(sample_dates(date(2026, 3, 1), date(2026, 2, 10)).is_empty());
        assert_eq!(
            sample_dates(date(2024, 2, 1), date(2024, 3, 31)),
            [date(2024, 2, 29), date(2024, 3, 31)]
        );
    }

    #[test]
    fn history_counts_runs_active_at_each_sample() {
        let runs = [
            run(1, date(2026, 1, 5), date(2026, 2, 15), false),
            run(2, date(2026, 1, 31), date(2026, 3, 31), true),
            run(3, date(2026, 2, 28), date(2026, 3, 1), false),
            run(1, date(2026, 3, 1), date(2027, 1, 1), true),
        ];
        let found = history(&runs, date(2026, 3, 10), &count);
        assert_eq!(
            found,
            [
                (date(2026, 1, 31), 2.0),
                (date(2026, 2, 28), 2.0),
                (date(2026, 3, 10), 2.0),
            ]
        );
        let at_end = history(&runs, date(2026, 3, 31), &count);
        assert_eq!(
            at_end.last(),
            Some(&(date(2026, 3, 31), 1.0)),
            "end is exclusive"
        );
    }

    #[test]
    fn new_adopters_need_a_start_within_365_days() {
        let today = date(2026, 3, 1);
        let runs = [
            run(1, date(2025, 3, 1), date(2027, 1, 1), true),
            run(2, date(2025, 3, 2), date(2027, 1, 1), true),
            run(3, date(2026, 3, 1), date(2027, 1, 1), true),
        ];
        let found = usage(&runs, today, &count);
        assert_eq!(found.current, 3.0);
        assert_eq!(
            found.new_adopters, 2.0,
            "2025-03-01 is 365 days back, outside"
        );
    }

    #[test]
    fn ended_runs_split_into_removed_and_expired() {
        let today = date(2026, 3, 1);
        let runs = [
            run(1, date(2025, 1, 1), date(2025, 6, 1), false),
            run(2, date(2025, 1, 1), date(2026, 3, 1), true),
            run(3, date(2025, 1, 1), date(2026, 3, 2), true),
            run(4, date(2025, 1, 1), date(2026, 1, 1), false),
        ];
        let found = usage(&runs, today, &count);
        assert_eq!((found.removed, found.expired), (2, 1));
        assert_eq!(found.current, 1.0);
        assert!(found.lost_users());
    }

    #[test]
    fn no_runs_give_neutral_signals() {
        let found = usage(&[], date(2026, 3, 1), &count);
        assert_eq!(
            found,
            Usage {
                current: 0.0,
                peak: 0.0,
                former_glory: 1.0,
                growth: 1.0,
                new_adopters: 0.0,
                removed: 0,
                expired: 0,
                history: Vec::new(),
            }
        );
        assert!(!found.lost_users());
    }
}
