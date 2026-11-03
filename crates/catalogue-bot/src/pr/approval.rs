//! The approval behind the `approved` label, read from the issue events.

use catalogue_core::{Date, GithubLogin};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::comment::APPROVED;
use crate::convert::Approval;

/// The event name of a label add.
const LABELED: &str = "labeled";
/// The event name of a label removal.
const UNLABELED: &str = "unlabeled";
/// The account type of a person.
const USER: &str = "User";

/// Why the pass waits: the events do not show the label add yet.
const LAGGING: &str =
    "the issue events do not show the current `approved` label yet. The next pass reads them again";
/// Why the pass waits: the issue carries no `approved` label.
const NO_LABEL: &str = "the issue has no `approved` label";
/// Why the approval is refused: no person added the label.
const NOT_USER: &str = "the latest `approved` label came from an account that is not a person. A maintainer must add it";

/// The account behind an issue event, as the API reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventActor {
    /// The account login.
    pub login: String,
    /// The account type: `User`, `Bot` or `Organization`.
    pub kind: String,
}

/// One issue event, as the API reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueEvent {
    /// The event id.
    pub id: u64,
    /// The event name, such as `labeled`.
    pub event: String,
    /// The account that caused the event. `None` when the API sends none.
    pub actor: Option<EventActor>,
    /// The label name of a label event.
    pub label: Option<String>,
    /// When the event happened.
    pub created_at: Timestamp,
}

/// The label add that approves an issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalEvent {
    /// The person who added the label.
    pub approver: GithubLogin,
    /// When the label was added.
    pub at: Timestamp,
    /// The UTC date of `at`.
    pub date: Date,
}

impl ApprovalEvent {
    /// The approver and the date, as a judgment records them.
    pub fn approval(&self) -> Approval {
        Approval {
            approver: self.approver.clone(),
            date: self.date,
        }
    }
}

/// What the issue events say about the `approved` label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalSource {
    /// A person added the label, and no removal follows.
    Approved(ApprovalEvent),
    /// No usable approval yet. The pass skips the issue with this reason.
    Pending(&'static str),
    /// The latest add came from no person. The bot removes the label with this reason.
    NotUser(&'static str),
}

/// The approval in `events` of an issue that carries `approved` when `labeled`.
///
/// Events sort by creation time, then id. The latest `approved` event must
/// be an add, else the events lag the label and the result is pending. The
/// latest add wins. An earlier add never stands in for it.
pub fn approval_event(events: &[IssueEvent], labeled: bool) -> ApprovalSource {
    if !labeled {
        return ApprovalSource::Pending(NO_LABEL);
    }
    let mut found: Vec<&IssueEvent> = events
        .iter()
        .filter(|event| event.label.as_deref() == Some(APPROVED))
        .filter(|event| event.event == LABELED || event.event == UNLABELED)
        .collect();
    found.sort_by_key(|event| (event.created_at, event.id));
    let Some(last) = found.last().filter(|event| event.event == LABELED) else {
        return ApprovalSource::Pending(LAGGING);
    };
    let approver = last
        .actor
        .as_ref()
        .filter(|actor| actor.kind == USER)
        .and_then(|actor| GithubLogin::new(actor.login.as_str()).ok());
    match approver {
        Some(approver) => ApprovalSource::Approved(ApprovalEvent {
            approver,
            at: last.created_at,
            date: Date::from(last.created_at.to_zoned(TimeZone::UTC).date()),
        }),
        None => ApprovalSource::NotUser(NOT_USER),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    fn event(id: u64, name: &str, login: Option<(&str, &str)>, when: &str) -> IssueEvent {
        IssueEvent {
            id,
            event: name.to_owned(),
            actor: login.map(|(login, kind)| EventActor {
                login: login.to_owned(),
                kind: kind.to_owned(),
            }),
            label: Some(APPROVED.to_owned()),
            created_at: at(when),
        }
    }

    fn approver(source: &ApprovalSource) -> &str {
        match source {
            ApprovalSource::Approved(found) => found.approver.as_str(),
            other => panic!("not approved: {other:?}"),
        }
    }

    #[test]
    fn the_latest_add_wins_after_a_removal() {
        let events = [
            event(1, LABELED, Some(("bob", USER)), "2026-11-03T10:00:00Z"),
            event(2, UNLABELED, Some(("bob", USER)), "2026-11-03T11:00:00Z"),
            event(
                3,
                LABELED,
                Some(("carol", USER)),
                "2026-11-03T23:30:00-02:00",
            ),
        ];
        let found = approval_event(&events, true);
        assert_eq!(approver(&found), "carol");
        let ApprovalSource::Approved(found) = found else {
            panic!("approved");
        };
        assert_eq!(found.date, Date::new(2026, 11, 4).expect("date"));
        assert_eq!(found.approval().approver.as_str(), "carol");
    }

    #[test]
    fn a_later_add_replaces_an_earlier_one() {
        let events = [
            event(1, LABELED, Some(("bob", USER)), "2026-11-03T10:00:00Z"),
            event(2, LABELED, Some(("zed", USER)), "2026-11-03T11:00:00Z"),
        ];
        assert_eq!(approver(&approval_event(&events, true)), "zed");
    }

    #[test]
    fn a_removal_after_the_last_add_means_the_events_lag() {
        let events = [
            event(1, LABELED, Some(("bob", USER)), "2026-11-03T10:00:00Z"),
            event(2, UNLABELED, Some(("bob", USER)), "2026-11-03T11:00:00Z"),
        ];
        assert_eq!(
            approval_event(&events, true),
            ApprovalSource::Pending(LAGGING)
        );
    }

    #[test]
    fn a_label_without_an_event_waits() {
        assert_eq!(approval_event(&[], true), ApprovalSource::Pending(LAGGING));
        let mut other = event(1, LABELED, Some(("bob", USER)), "2026-11-03T10:00:00Z");
        other.label = Some("submit".to_owned());
        assert_eq!(
            approval_event(&[other], true),
            ApprovalSource::Pending(LAGGING)
        );
        assert_eq!(
            approval_event(&[], false),
            ApprovalSource::Pending(NO_LABEL)
        );
    }

    #[test]
    fn events_sort_by_time_then_id() {
        let events = [
            event(9, LABELED, Some(("carol", USER)), "2026-11-03T12:00:00Z"),
            event(4, UNLABELED, Some(("bob", USER)), "2026-11-03T11:00:00Z"),
            event(2, LABELED, Some(("bob", USER)), "2026-11-03T10:00:00Z"),
        ];
        assert_eq!(approver(&approval_event(&events, true)), "carol");
        let same_time = [
            event(8, LABELED, Some(("carol", USER)), "2026-11-03T12:00:00Z"),
            event(7, UNLABELED, Some(("bob", USER)), "2026-11-03T12:00:00Z"),
        ];
        assert_eq!(approver(&approval_event(&same_time, true)), "carol");
        let removal_last = [
            event(7, LABELED, Some(("carol", USER)), "2026-11-03T12:00:00Z"),
            event(8, UNLABELED, Some(("bob", USER)), "2026-11-03T12:00:00Z"),
        ];
        assert_eq!(
            approval_event(&removal_last, true),
            ApprovalSource::Pending(LAGGING)
        );
    }

    #[test]
    fn a_bot_or_missing_actor_is_refused_without_falling_back() {
        let bot = [
            event(1, LABELED, Some(("bob", USER)), "2026-11-03T10:00:00Z"),
            event(
                2,
                LABELED,
                Some(("helper[bot]", "Bot")),
                "2026-11-03T11:00:00Z",
            ),
        ];
        assert_eq!(
            approval_event(&bot, true),
            ApprovalSource::NotUser(NOT_USER)
        );
        let ghost = [
            event(1, LABELED, Some(("bob", USER)), "2026-11-03T10:00:00Z"),
            event(2, LABELED, None, "2026-11-03T11:00:00Z"),
        ];
        assert_eq!(
            approval_event(&ghost, true),
            ApprovalSource::NotUser(NOT_USER)
        );
    }
}
