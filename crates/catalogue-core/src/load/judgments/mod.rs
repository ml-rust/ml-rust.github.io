//! Checks for judgment files.

mod approval;
mod candidate;
mod check;
mod references;
mod supersedes;

pub(crate) use candidate::{
    approval_problems, eligible_approvers, reference_problems, submission_approvers,
};
pub(super) use check::{Refs, check_judgments};
