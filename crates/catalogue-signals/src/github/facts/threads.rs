//! Maintainer response facts from sampled issues and pull requests.

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;

use super::bot::is_bot_login;
use super::response::Item;

/// The author associations that make a person a maintainer.
const MAINTAINER_ASSOCIATIONS: [&str; 3] = ["OWNER", "MEMBER", "COLLABORATOR"];

/// The state of an item that still waits for an answer.
const OPEN_STATE: &str = "OPEN";

/// Seconds in a day, for the time from creation to the first answer.
const SECONDS_PER_DAY: i64 = 86_400;

/// The response facts of the sampled items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct ThreadFacts {
    /// The age in days of the oldest open non-member item with no maintainer comment.
    pub(super) oldest_unanswered_days: Option<u32>,
    /// The maintainer comments dated on or after the window start.
    pub(super) maintainer_replies: u32,
    /// The lower median days to the first maintainer comment, over non-member
    /// items created in the window that have one.
    pub(super) median_first_response_days: Option<u32>,
    /// The items sampled.
    pub(super) items_sampled: u32,
}

/// Whether an author association marks a maintainer.
fn is_maintainer(association: Option<&str>) -> bool {
    association.is_some_and(|value| MAINTAINER_ASSOCIATIONS.contains(&value))
}

/// The UTC date of a timestamp.
fn day_of(when: Timestamp) -> Date {
    when.to_zoned(TimeZone::UTC).date()
}

/// The whole days from `from` to `to`, zero when `to` is earlier.
fn days_between(from: Date, to: Date) -> u32 {
    from.until(to)
        .ok()
        .and_then(|span| u32::try_from(span.get_days()).ok())
        .unwrap_or(0)
}

/// The lower median of `values`, or `None` when empty.
fn lower_median(mut values: Vec<u32>) -> Option<u32> {
    values.sort_unstable();
    values.get(values.len().checked_sub(1)? / 2).copied()
}

/// Derives the response facts from `items`.
///
/// `today` dates the age of unanswered items. `window_start` is the first day
/// of the 365-day window. An item with an unreadable creation date counts as
/// sampled and adds nothing else.
pub(super) fn thread_facts<'a>(
    items: impl IntoIterator<Item = &'a Item>,
    today: Date,
    window_start: Date,
) -> ThreadFacts {
    let mut facts = ThreadFacts::default();
    let mut responses = Vec::new();
    for item in items {
        facts.items_sampled = facts.items_sampled.saturating_add(1);
        let Ok(created) = item.created_at.parse::<Timestamp>() else {
            continue;
        };
        let mut first_answer: Option<Timestamp> = None;
        for comment in item.comments.iter().flat_map(|c| c.items()) {
            if !is_maintainer(comment.author_association.as_deref()) {
                continue;
            }
            let Ok(when) = comment.created_at.parse::<Timestamp>() else {
                continue;
            };
            if day_of(when) >= window_start {
                facts.maintainer_replies = facts.maintainer_replies.saturating_add(1);
            }
            first_answer = Some(first_answer.map_or(when, |earlier| earlier.min(when)));
        }
        let by_member = is_maintainer(item.author_association.as_deref());
        let by_bot = item
            .author
            .as_ref()
            .is_some_and(|author| is_bot_login(&author.login));
        if by_member || by_bot {
            continue;
        }
        let created_day = day_of(created);
        match first_answer {
            None if item.state == OPEN_STATE => {
                let age = days_between(created_day, today);
                facts.oldest_unanswered_days = facts.oldest_unanswered_days.max(Some(age));
            }
            Some(answer) if created_day >= window_start => {
                let seconds = answer.as_second().saturating_sub(created.as_second());
                let days = u32::try_from(seconds.max(0) / SECONDS_PER_DAY).unwrap_or(u32::MAX);
                responses.push(days);
            }
            _ => {}
        }
    }
    facts.median_first_response_days = lower_median(responses);
    facts
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use serde_json::{Value, json};

    use super::*;

    const TODAY: Date = date(2026, 11, 3);
    const START: Date = date(2025, 11, 3);

    fn comment(when: &str, association: &str) -> Value {
        json!({"createdAt": when, "authorAssociation": association, "author": {"login": "c"}})
    }

    fn item(
        created: &str,
        state: &str,
        association: &str,
        login: &str,
        comments: &[Value],
    ) -> Item {
        serde_json::from_value(json!({
            "createdAt": created,
            "state": state,
            "authorAssociation": association,
            "author": {"login": login},
            "comments": {"nodes": comments},
        }))
        .expect("item")
    }

    fn facts(items: &[Item]) -> ThreadFacts {
        thread_facts(items, TODAY, START)
    }

    #[test]
    fn the_oldest_open_unanswered_non_member_item_sets_the_age() {
        let items = [
            item("2026-10-24T00:00:00Z", "OPEN", "NONE", "u1", &[]),
            item("2026-09-24T12:00:00Z", "OPEN", "CONTRIBUTOR", "u2", &[]),
            item("2026-01-01T00:00:00Z", "OPEN", "OWNER", "m", &[]),
            item("2026-01-01T00:00:00Z", "OPEN", "NONE", "dependabot", &[]),
            item("2026-01-01T00:00:00Z", "CLOSED", "NONE", "u3", &[]),
            item(
                "2026-01-01T00:00:00Z",
                "OPEN",
                "NONE",
                "u4",
                &[comment("2026-01-02T00:00:00Z", "MEMBER")],
            ),
        ];
        let result = facts(&items);
        assert_eq!(result.oldest_unanswered_days, Some(40));
        assert_eq!(result.items_sampled, 6);
    }

    #[test]
    fn a_non_maintainer_comment_does_not_answer() {
        let items = [item(
            "2026-10-24T00:00:00Z",
            "OPEN",
            "NONE",
            "u1",
            &[comment("2026-10-25T00:00:00Z", "NONE")],
        )];
        assert_eq!(facts(&items).oldest_unanswered_days, Some(10));
    }

    #[test]
    fn no_open_unanswered_item_gives_none() {
        assert_eq!(facts(&[]).oldest_unanswered_days, None);
        assert_eq!(facts(&[]).items_sampled, 0);
    }

    #[test]
    fn maintainer_replies_count_only_inside_the_window() {
        let items = [item(
            "2024-01-01T00:00:00Z",
            "CLOSED",
            "NONE",
            "u",
            &[
                comment("2025-11-02T23:59:59Z", "OWNER"),
                comment("2025-11-03T00:00:00Z", "MEMBER"),
                comment("2026-05-01T00:00:00Z", "COLLABORATOR"),
                comment("2026-05-02T00:00:00Z", "CONTRIBUTOR"),
            ],
        )];
        assert_eq!(facts(&items).maintainer_replies, 2);
    }

    fn answered(created: &str, answer: &str) -> Item {
        item(created, "CLOSED", "NONE", "u", &[comment(answer, "OWNER")])
    }

    #[test]
    fn the_median_of_an_odd_count_is_the_middle() {
        let items = [
            answered("2026-10-01T00:00:00Z", "2026-10-02T00:00:00Z"),
            answered("2026-10-01T00:00:00Z", "2026-10-11T00:00:00Z"),
            answered("2026-10-01T00:00:00Z", "2026-10-04T12:00:00Z"),
        ];
        assert_eq!(facts(&items).median_first_response_days, Some(3));
    }

    #[test]
    fn the_median_of_an_even_count_is_the_lower_middle() {
        let items = [
            answered("2026-10-01T00:00:00Z", "2026-10-02T00:00:00Z"),
            answered("2026-10-01T00:00:00Z", "2026-10-11T00:00:00Z"),
        ];
        assert_eq!(facts(&items).median_first_response_days, Some(1));
    }

    #[test]
    fn no_answered_item_in_the_window_gives_no_median() {
        let items = [
            answered("2025-01-01T00:00:00Z", "2025-01-02T00:00:00Z"),
            item("2026-10-01T00:00:00Z", "OPEN", "NONE", "u", &[]),
            item(
                "2026-10-01T00:00:00Z",
                "CLOSED",
                "MEMBER",
                "m",
                &[comment("2026-10-02T00:00:00Z", "OWNER")],
            ),
        ];
        assert_eq!(facts(&items).median_first_response_days, None);
    }

    #[test]
    fn the_first_maintainer_comment_sets_the_response_time() {
        let items = [item(
            "2026-10-01T00:00:00Z",
            "CLOSED",
            "NONE",
            "u",
            &[
                comment("2026-10-09T00:00:00Z", "OWNER"),
                comment("2026-10-03T00:00:00Z", "MEMBER"),
            ],
        )];
        assert_eq!(facts(&items).median_first_response_days, Some(2));
    }

    #[test]
    fn median_helper_handles_empty_and_single() {
        assert_eq!(lower_median(Vec::new()), None);
        assert_eq!(lower_median(vec![7]), Some(7));
        assert_eq!(lower_median(vec![9, 1, 5, 3]), Some(3));
    }
}
