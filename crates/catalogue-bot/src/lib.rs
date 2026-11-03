//! Intake for the ml-rust catalogue bot: issue form bodies into data records and check reports.
//!
//! The crate reads no files, opens no connections, reads no clock and no
//! environment. Issue text is untrusted. [`parse_form`] accepts only the
//! form's own sections, [`submission`] and [`judgment_draft`] build the
//! records, and [`check_submission`] reports every problem as markdown in
//! which issue text stays inside fenced blocks. [`parse_labels`] reads the
//! repo labels file and [`plan_sync`] plans the label sync.

mod check;
mod convert;
mod form;
mod labels;

pub use check::{Block, CheckInput, CheckReport, Section, check_submission};
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
