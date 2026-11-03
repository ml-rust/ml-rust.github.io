//! Approved issues into data PRs: the approval, the ownership rules, the decision and the PR text.

mod approval;
mod decision;
mod existing;
mod ownership;
mod prepare;
mod text;

pub use approval::{ApprovalEvent, ApprovalSource, EventActor, IssueEvent, approval_event};
pub use decision::{PrDecision, PrInput, PrIssue, PrPlan, pr_decision, precheck};
pub use existing::{ExistingPr, PullFacts, existing_pr};
pub use ownership::{
    Account, Basis, CrateOwner, CrateOwners, OwnerFacts, Ownership, RepoFacts, TeamOwner, decide,
    membership_org,
};
pub use prepare::{Prepared, prepare};
pub use text::{BRANCH_PREFIX, branch_name};
