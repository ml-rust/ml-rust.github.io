//! The input, the outcome and the cheap rules of the PR decision.

use catalogue_core::{DataSources, Date};

use super::approval::{ApprovalEvent, ApprovalSource};
use super::ownership::Ownership;
use super::prepare::prepare;
use crate::comment::{APPROVED, Sha256};
use crate::convert::IssueMeta;
use crate::form::FormKind;

/// The issue as a fresh `GET /issues/{n}` reports it.
#[derive(Debug, Clone, Copy)]
pub struct PrIssue<'a> {
    /// Whether the issue is open.
    pub open: bool,
    /// The label names.
    pub labels: &'a [String],
    /// The body exactly as the API returns it.
    pub body: &'a str,
}

/// Everything the PR decision reads. The caller fetches it.
#[derive(Debug, Clone, Copy)]
pub struct PrInput<'a> {
    /// The issue number and author.
    pub meta: &'a IssueMeta,
    /// The issue, fetched again just before the decision.
    pub issue: PrIssue<'a>,
    /// Whether the check report of this pass blocks.
    pub blocking: bool,
    /// The digest of the body the check of this pass read.
    pub checked_body: Sha256,
    /// The approval from the issue events.
    pub approval: &'a ApprovalSource,
    /// The data files at the current head of `main`.
    pub main: &'a DataSources,
    /// The date that becomes `added` on a new entry.
    pub today: Date,
}

/// The PR the bot opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrPlan {
    /// The branch: `bot/issue-<n>`.
    pub branch: String,
    /// The file path relative to the repository root.
    pub path: String,
    /// The file text.
    pub contents: String,
    /// The PR title.
    pub title: String,
    /// The commit message.
    pub commit_message: String,
    /// The PR body.
    pub body: String,
}

/// What the bot does with an approved issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrDecision {
    /// Nothing this pass, for this reason.
    Skip(String),
    /// No PR until these problems are fixed.
    Blocked(Vec<String>),
    /// The approval rules refuse the approver. The bot removes `approved`.
    RemoveApproval(Vec<String>),
    /// Open this PR.
    Open(Box<PrPlan>),
}

/// The rules that need no fetch: the issue is open, has one form label and
/// `approved`, the report does not block, and a person approved it.
///
/// Returns the form and the approval when every rule holds.
///
/// # Errors
///
/// Returns the decision that stops the PR: a skip, or the approval removal.
pub fn precheck<'a>(
    open: bool,
    labels: &[String],
    blocking: bool,
    approval: &'a ApprovalSource,
) -> Result<(FormKind, &'a ApprovalEvent), PrDecision> {
    let skip = |reason: &str| PrDecision::Skip(reason.to_owned());
    if !open {
        return Err(skip("the issue is closed"));
    }
    let kind = FormKind::from_labels(labels.iter().map(String::as_str))
        .map_err(|_| skip("the issue needs exactly one form label"))?;
    if !labels.iter().any(|label| label == APPROVED) {
        return Err(skip("the issue has no `approved` label"));
    }
    if blocking {
        return Err(skip("the check report blocks"));
    }
    match approval {
        ApprovalSource::Approved(event) => Ok((kind, event)),
        ApprovalSource::Pending(reason) => Err(skip(reason)),
        ApprovalSource::NotUser(reason) => {
            Err(PrDecision::RemoveApproval(vec![(*reason).to_owned()]))
        }
    }
}

/// The full decision for `input`, with the `ownership` result of a maintainer statement.
pub fn pr_decision(input: &PrInput<'_>, ownership: Option<&Ownership>) -> PrDecision {
    match prepare(input) {
        Ok(prepared) => prepared.finish(ownership),
        Err(decision) => decision,
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::GithubLogin;

    use super::*;

    fn labels(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn approved() -> ApprovalSource {
        ApprovalSource::Approved(ApprovalEvent {
            approver: GithubLogin::new("bob").expect("login"),
            at: "2026-11-04T09:00:00Z".parse().expect("time"),
            date: Date::new(2026, 11, 4).expect("date"),
        })
    }

    fn skip(reason: &str) -> Option<PrDecision> {
        Some(PrDecision::Skip(reason.to_owned()))
    }

    #[test]
    fn every_cheap_rule_must_hold() {
        let good = labels(&["submit", "approved"]);
        let approval = approved();
        let found = precheck(true, &good, false, &approval).expect("passes");
        assert_eq!(found.0, FormKind::Submit);
        assert_eq!(found.1.approver.as_str(), "bob");
        let stop = |open, names: &[&str], blocking, approval: &ApprovalSource| {
            precheck(open, &labels(names), blocking, approval).err()
        };
        let both = ["submit", "approved"];
        assert_eq!(
            stop(false, &both, false, &approval),
            skip("the issue is closed")
        );
        assert_eq!(
            stop(
                true,
                &["submit", "judgment:kind", "approved"],
                false,
                &approval
            ),
            skip("the issue needs exactly one form label")
        );
        assert_eq!(
            stop(true, &["submit"], false, &approval),
            skip("the issue has no `approved` label")
        );
        assert_eq!(
            stop(true, &both, true, &approval),
            skip("the check report blocks")
        );
        assert_eq!(
            stop(true, &both, false, &ApprovalSource::Pending("wait")),
            skip("wait")
        );
        assert_eq!(
            stop(true, &both, false, &ApprovalSource::NotUser("not a person")),
            Some(PrDecision::RemoveApproval(vec!["not a person".to_owned()]))
        );
    }
}
