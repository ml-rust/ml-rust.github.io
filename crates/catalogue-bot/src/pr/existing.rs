//! What a PR already on the bot branch means for the current approval.

use jiff::Timestamp;

use super::approval::ApprovalEvent;

/// One PR from the bot branch of the issue, as the API lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullFacts {
    /// The PR number.
    pub number: u64,
    /// Whether the PR is open.
    pub open: bool,
    /// Whether the PR is merged.
    pub merged: bool,
    /// When the PR closed. `None` while open.
    pub closed_at: Option<Timestamp>,
}

/// The PR state that decides whether the bot opens a new PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingPr {
    /// No PR blocks a new one.
    None,
    /// This PR is open. The bot writes nothing.
    Open(u64),
    /// This PR is merged. The bot writes nothing.
    Merged(u64),
    /// This PR closed without a merge, after the current approval. A new
    /// approval retries.
    Closed(u64),
}

/// The state of the `pulls` on the bot branch for `approval`.
///
/// An open PR wins, then a merged one. Closed PRs block a new one unless the
/// approval is newer than every close. A closed PR with no close time blocks.
pub fn existing_pr(pulls: &[PullFacts], approval: &ApprovalEvent) -> ExistingPr {
    if let Some(found) = pulls.iter().find(|pull| pull.open) {
        return ExistingPr::Open(found.number);
    }
    if let Some(found) = pulls.iter().find(|pull| pull.merged) {
        return ExistingPr::Merged(found.number);
    }
    let blocking = pulls
        .iter()
        .filter(|pull| pull.closed_at.is_none_or(|closed| closed >= approval.at))
        .max_by_key(|pull| pull.number);
    match blocking {
        Some(found) => ExistingPr::Closed(found.number),
        None => ExistingPr::None,
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, GithubLogin};

    use super::*;

    fn at(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    fn approval(when: &str) -> ApprovalEvent {
        ApprovalEvent {
            approver: GithubLogin::new("bob").expect("login"),
            at: at(when),
            date: Date::new(2026, 11, 3).expect("date"),
        }
    }

    fn pull(number: u64, open: bool, merged: bool, closed: Option<&str>) -> PullFacts {
        PullFacts {
            number,
            open,
            merged,
            closed_at: closed.map(at),
        }
    }

    #[test]
    fn open_then_merged_wins() {
        let now = approval("2026-11-03T12:00:00Z");
        let open = [
            pull(3, false, true, Some("2026-11-01T00:00:00Z")),
            pull(5, true, false, None),
        ];
        assert_eq!(existing_pr(&open, &now), ExistingPr::Open(5));
        let merged = [
            pull(3, false, false, Some("2026-11-01T00:00:00Z")),
            pull(4, false, true, Some("2026-11-02T00:00:00Z")),
        ];
        assert_eq!(existing_pr(&merged, &now), ExistingPr::Merged(4));
        assert_eq!(existing_pr(&[], &now), ExistingPr::None);
    }

    #[test]
    fn a_closed_pr_blocks_until_a_newer_approval() {
        let closed = [pull(4, false, false, Some("2026-11-03T10:00:00Z"))];
        assert_eq!(
            existing_pr(&closed, &approval("2026-11-03T09:00:00Z")),
            ExistingPr::Closed(4)
        );
        assert_eq!(
            existing_pr(&closed, &approval("2026-11-03T11:00:00Z")),
            ExistingPr::None
        );
        let no_time = [pull(6, false, false, None)];
        assert_eq!(
            existing_pr(&no_time, &approval("2026-11-03T11:00:00Z")),
            ExistingPr::Closed(6)
        );
    }
}
