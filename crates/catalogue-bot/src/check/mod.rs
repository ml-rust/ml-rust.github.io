//! The deterministic check report of one issue.

mod notes;
mod report;
mod submission;
mod validation;

pub use report::{Block, CheckReport, Section};
pub use submission::{CheckInput, check_submission};
