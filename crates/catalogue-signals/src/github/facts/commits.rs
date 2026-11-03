//! Human commit counts from the default branch history.

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;

use super::bot::is_bot_commit;
use super::query::{COMMIT_PAGE, LATEST_SAMPLE};
use super::response::History;

/// The human commit facts of one history window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct CommitFacts {
    /// The date of the newest human commit.
    pub(super) last_human: Option<Date>,
    /// The human commits among the commits read.
    pub(super) human_count: u32,
    /// Whether the window holds more commits than were read.
    pub(super) capped: bool,
}

/// The UTC dates of the human commits in `history`.
///
/// A commit with an unreadable date is skipped.
fn human_days(history: &History) -> impl Iterator<Item = Date> + '_ {
    history
        .nodes
        .iter()
        .flatten()
        .flatten()
        .filter_map(|commit| {
            let (login, name) = commit.author.as_ref().map_or((None, None), |author| {
                (
                    author.user.as_ref().map(|user| user.login.as_str()),
                    author.name.as_deref(),
                )
            });
            if is_bot_commit(login, name) {
                return None;
            }
            let when = commit.committed_date.parse::<Timestamp>().ok()?;
            Some(when.to_zoned(TimeZone::UTC).date())
        })
}

/// Counts the human commits in `history`.
///
/// The count is a lower bound when [`CommitFacts::capped`] is set. A commit
/// with an unreadable date counts toward the total and not toward the human
/// facts.
pub(super) fn commit_facts(history: &History) -> CommitFacts {
    let mut facts = CommitFacts {
        capped: history.total_count > COMMIT_PAGE,
        ..CommitFacts::default()
    };
    for day in human_days(history) {
        facts.human_count = facts.human_count.saturating_add(1);
        facts.last_human = facts.last_human.max(Some(day));
    }
    facts
}

/// The newest human commit of a repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct LastHuman {
    /// The date of the newest human commit that the two histories hold.
    pub(super) date: Option<Date>,
    /// Whether no human commit shows and the unbounded sample is full of bots.
    pub(super) beyond_sample: bool,
}

/// Finds the newest human commit across the window and the latest sample.
///
/// `window` holds the facts of the dated window. `latest` holds the newest
/// [`LATEST_SAMPLE`] commits of any age. The sample is full of bots when the
/// branch holds more commits than the sample read, and none of them is human.
pub(super) fn last_human_commit(window: &CommitFacts, latest: Option<&History>) -> LastHuman {
    let in_sample = latest.and_then(|history| human_days(history).max());
    let date = window.last_human.max(in_sample);
    let beyond_sample =
        date.is_none() && latest.is_some_and(|history| history.total_count > LATEST_SAMPLE);
    LastHuman {
        date,
        beyond_sample,
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use serde_json::json;

    use super::*;

    fn history(total: u64, commits: &[(&str, Option<&str>, Option<&str>)]) -> History {
        let nodes: Vec<_> = commits
            .iter()
            .map(|(when, login, name)| {
                json!({
                    "committedDate": when,
                    "author": {"user": login.map(|l| json!({"login": l})), "name": name},
                })
            })
            .collect();
        serde_json::from_value(json!({"totalCount": total, "nodes": nodes})).expect("history")
    }

    #[test]
    fn bots_do_not_count_and_the_newest_human_date_wins() {
        let facts = commit_facts(&history(
            4,
            &[
                ("2026-10-30T10:00:00Z", Some("dependabot[bot]"), None),
                ("2026-10-01T23:59:59Z", Some("alice"), Some("Alice")),
                ("2026-09-01T00:00:00Z", Some("bob"), None),
                ("2026-10-29T00:00:00Z", None, Some("renovate")),
            ],
        ));
        assert_eq!(facts.human_count, 2);
        assert_eq!(facts.last_human, Some(date(2026, 10, 1)));
        assert!(!facts.capped);
    }

    #[test]
    fn more_than_one_page_marks_the_count_as_capped() {
        let at_cap = commit_facts(&history(100, &[("2026-10-01T00:00:00Z", Some("a"), None)]));
        assert!(!at_cap.capped);
        let over = commit_facts(&history(101, &[("2026-10-01T00:00:00Z", Some("a"), None)]));
        assert!(over.capped);
        assert_eq!(over.human_count, 1);
    }

    #[test]
    fn an_empty_window_has_no_human_commit() {
        let facts = commit_facts(&history(0, &[]));
        assert_eq!(facts, CommitFacts::default());
    }

    #[test]
    fn an_old_human_commit_in_the_sample_sets_the_date() {
        let latest = history(
            500,
            &[
                ("2026-10-30T10:00:00Z", Some("dependabot[bot]"), None),
                ("2024-08-01T12:00:00Z", Some("alice"), Some("Alice")),
            ],
        );
        let found = last_human_commit(&CommitFacts::default(), Some(&latest));
        assert_eq!(found.date, Some(date(2024, 8, 1)));
        assert!(!found.beyond_sample);
    }

    #[test]
    fn the_window_wins_over_an_older_sample_date() {
        let window = CommitFacts {
            last_human: Some(date(2026, 10, 1)),
            ..CommitFacts::default()
        };
        let latest = history(5, &[("2024-08-01T12:00:00Z", Some("alice"), None)]);
        let found = last_human_commit(&window, Some(&latest));
        assert_eq!(found.date, Some(date(2026, 10, 1)));
    }

    #[test]
    fn a_full_sample_of_bots_is_beyond_the_sample() {
        let bots: Vec<_> = (0..LATEST_SAMPLE)
            .map(|_| ("2026-10-30T10:00:00Z", Some("dependabot[bot]"), None))
            .collect();
        let found = last_human_commit(&CommitFacts::default(), Some(&history(500, &bots)));
        let expected = LastHuman {
            date: None,
            beyond_sample: true,
        };
        assert_eq!(found, expected);
        let all = history(LATEST_SAMPLE, &bots);
        let short = last_human_commit(&CommitFacts::default(), Some(&all));
        assert_eq!(short, LastHuman::default());
        let none = last_human_commit(&CommitFacts::default(), None);
        assert_eq!(none, LastHuman::default());
    }

    #[test]
    fn a_commit_with_no_author_is_human() {
        let history: History = serde_json::from_value(json!({
            "totalCount": 1,
            "nodes": [{"committedDate": "2026-10-01T00:00:00Z", "author": null}, null],
        }))
        .expect("history");
        assert_eq!(commit_facts(&history).human_count, 1);
    }
}
