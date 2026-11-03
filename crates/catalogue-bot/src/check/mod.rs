//! The deterministic check report of one issue.

mod label;
mod notes;
mod report;
mod submission;
mod validation;

pub use label::label_report;
pub use report::{Block, CheckReport, Section};
pub use submission::{CheckInput, check_submission};
