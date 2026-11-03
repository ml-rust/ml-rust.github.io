//! The writes one issue needs, from the desired check and the previous bot comment.

use super::digest::Sha256;
use super::marker::Marker;
use super::names::{APPROVED, BOT_CHECKED, NEEDS_FIX};

/// The check the bot wants on an issue now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Desired {
    /// The marker of the check: body, report and `main`.
    pub marker: Marker,
    /// Whether the report blocks the issue.
    pub blocking: bool,
    /// Whether the comment carries a one-time note, so a matching marker still gets a rewrite.
    pub note: bool,
}

/// The bot comment already on the issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Previous {
    /// The comment id.
    pub comment_id: u64,
    /// The marker on its first line.
    pub marker: Marker,
}

/// What to do with the bot comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentAction {
    /// The comment covers the desired check. Write nothing.
    Skip,
    /// The issue has no bot comment. Post one.
    Create,
    /// Write the new comment over the comment with this id.
    Update(u64),
}

/// Every write one issue needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actions {
    /// The comment write.
    pub comment: CommentAction,
    /// Labels to add, in order.
    pub add_labels: Vec<String>,
    /// Labels to remove, in order. `approved` is never here: see `drop_approved`.
    pub remove_labels: Vec<String>,
    /// Whether to remove `approved`, because it covers a body the bot never checked.
    pub drop_approved: bool,
}

/// Whether `approved` covers a body the bot never checked: the issue carries
/// it, and the `previous` marker covers a body other than `body`.
pub fn approval_stale(
    previous: Option<&Previous>,
    body: &Sha256,
    current_labels: &[String],
) -> bool {
    current_labels.iter().any(|label| label == APPROVED)
        && previous.is_some_and(|found| found.marker.body != *body)
}

/// Plans the writes for one issue that carries `current_labels`.
///
/// - Labels: `needs-fix` when the report blocks, else `bot-checked`. The other one goes.
/// - Comment: post one when `previous` is `None`. Rewrite it when its marker
///   differs from the desired one in the body, the report or `main`, or when
///   the desired comment carries a note. Skip otherwise.
/// - Approval: drop `approved` when [`approval_stale`] holds. A maintainer
///   approved text the bot has not checked.
pub fn plan(desired: &Desired, previous: Option<&Previous>, current_labels: &[String]) -> Actions {
    let has = |name: &str| current_labels.iter().any(|label| label == name);
    let (wanted, other) = if desired.blocking {
        (NEEDS_FIX, BOT_CHECKED)
    } else {
        (BOT_CHECKED, NEEDS_FIX)
    };
    let add_labels = if has(wanted) {
        Vec::new()
    } else {
        vec![wanted.to_owned()]
    };
    let remove_labels = if has(other) {
        vec![other.to_owned()]
    } else {
        Vec::new()
    };
    let comment = match previous {
        None => CommentAction::Create,
        Some(found) if found.marker == desired.marker && !desired.note => CommentAction::Skip,
        Some(found) => CommentAction::Update(found.comment_id),
    };
    let drop_approved = approval_stale(previous, &desired.marker.body, current_labels);
    Actions {
        comment,
        add_labels,
        remove_labels,
        drop_approved,
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::CommitSha;

    use super::*;

    fn marker(body: &str, report: &str, main: Option<&str>) -> Marker {
        Marker {
            body: Sha256::of(body.as_bytes()),
            report: Sha256::of(report.as_bytes()),
            main: main.map(|sha| CommitSha::new(sha.repeat(40)).expect("sha")),
        }
    }

    fn desired(blocking: bool) -> Desired {
        Desired {
            marker: marker("b", "r", Some("a")),
            blocking,
            note: false,
        }
    }

    fn previous(found: Marker) -> Previous {
        Previous {
            comment_id: 9,
            marker: found,
        }
    }

    fn labels(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn a_first_check_posts_and_labels() {
        let actions = plan(&desired(false), None, &labels(&["submit"]));
        assert_eq!(
            actions,
            Actions {
                comment: CommentAction::Create,
                add_labels: labels(&["bot-checked"]),
                remove_labels: Vec::new(),
                drop_approved: false,
            }
        );
        let blocking = plan(&desired(true), None, &labels(&["submit", "bot-checked"]));
        assert_eq!(blocking.add_labels, labels(&["needs-fix"]));
        assert_eq!(blocking.remove_labels, labels(&["bot-checked"]));
    }

    #[test]
    fn an_unchanged_check_writes_nothing() {
        let same = previous(marker("b", "r", Some("a")));
        let actions = plan(
            &desired(false),
            Some(&same),
            &labels(&["submit", "bot-checked"]),
        );
        assert_eq!(actions.comment, CommentAction::Skip);
        assert!(actions.add_labels.is_empty() && actions.remove_labels.is_empty());
        assert!(!actions.drop_approved);
    }

    #[test]
    fn a_change_in_body_report_or_main_rewrites_the_comment() {
        for old in [
            marker("old", "r", Some("a")),
            marker("b", "old", Some("a")),
            marker("b", "r", Some("c")),
            marker("b", "r", None),
        ] {
            let actions = plan(
                &desired(false),
                Some(&previous(old)),
                &labels(&["bot-checked"]),
            );
            assert_eq!(actions.comment, CommentAction::Update(9));
        }
    }

    #[test]
    fn a_note_rewrites_a_matching_comment() {
        let same = previous(marker("b", "r", Some("a")));
        let mut noted = desired(false);
        noted.note = true;
        let actions = plan(&noted, Some(&same), &labels(&["bot-checked"]));
        assert_eq!(actions.comment, CommentAction::Update(9));
    }

    #[test]
    fn a_removed_label_is_added_again() {
        let same = previous(marker("b", "r", Some("a")));
        let actions = plan(&desired(false), Some(&same), &labels(&["submit"]));
        assert_eq!(actions.comment, CommentAction::Skip);
        assert_eq!(actions.add_labels, labels(&["bot-checked"]));
    }

    #[test]
    fn approval_of_an_unchecked_body_is_dropped() {
        let current = labels(&["submit", "approved", "bot-checked"]);
        let edited = previous(marker("old", "r", Some("a")));
        assert!(plan(&desired(false), Some(&edited), &current).drop_approved);
        let new_main = previous(marker("b", "r", Some("c")));
        assert!(!plan(&desired(false), Some(&new_main), &current).drop_approved);
        assert!(!plan(&desired(false), None, &current).drop_approved);
        let unapproved = labels(&["submit"]);
        assert!(!plan(&desired(false), Some(&edited), &unapproved).drop_approved);
        let actions = plan(&desired(false), Some(&edited), &current);
        assert!(!actions.remove_labels.iter().any(|name| name == "approved"));
    }
}
