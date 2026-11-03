//! Intake for the ml-rust catalogue bot: issue form bodies into data records and check reports.
//!
//! The crate reads no files, opens no connections, reads no clock and no
//! environment. Issue text is untrusted. [`parse_form`] accepts only the
//! form's own sections, [`submission`] and [`judgment_draft`] build the
//! records, and [`check_submission`] reports every problem as markdown in
//! which issue text stays inside fenced blocks. [`parse_labels`] reads the
//! repo labels file and [`plan_sync`] plans the label sync. [`plan`] decides the
//! comment and label writes for one issue from the [`Marker`] of the last bot
//! comment, and [`comment_body`] builds the comment text within GitHub's cap.
//! [`approval_event`] reads the approval from the issue events, [`decide`]
//! applies the ownership rules, and [`pr_decision`] turns an approved issue
//! into the data PR it becomes.

mod check;
mod comment;
mod convert;
mod form;
mod labels;
mod pr;

pub use check::{Block, CheckInput, CheckReport, Section, check_submission, label_report};
pub use comment::{
    APPROVED, Actions, BOT_CHECKED, CommentAction, Desired, MAX_COMMENT_BYTES, Marker, NEEDS_FIX,
    Note, PR_OPEN, PrStatus, Previous, Sha256, approval_stale, comment_body, plan, report_digest,
};
pub use convert::{
    Affiliation, Approval, ENTRIES_DIR, IssueMeta, JUDGMENTS_DIR, JudgmentDraft, PrNotes,
    SlugSource, Submission, draft_toml, entry_toml, judgment, judgment_draft, judgment_toml,
    submission,
};
pub use form::{
    AFFILIATED, CATEGORY, CORRECTNESS, DEMOTION, FEATURED_APPROVAL, Field, Form, FormError,
    FormKind, JudgmentAnswers, JudgmentForm, KIND, LabelError, MAINTAINER, NOT_AFFILIATED, SUBMIT,
    SUPERSEDES, Shape, SubmitForm, parse_form,
};
pub use labels::{
    DESCRIPTION_MAX, LabelProblem, LabelSpec, LabelUpdate, LabelsError, LabelsFile, RepoLabel,
    SyncPlan, parse_labels, plan_sync,
};
pub use pr::{
    Account, ApprovalEvent, ApprovalSource, BRANCH_PREFIX, Basis, CrateOwner, CrateOwners,
    EventActor, ExistingPr, IssueEvent, OwnerFacts, Ownership, PrDecision, PrInput, PrIssue,
    PrPlan, Prepared, PullFacts, RepoFacts, TeamOwner, approval_event, branch_name, decide,
    existing_pr, membership_org, pr_decision, precheck, prepare,
};
