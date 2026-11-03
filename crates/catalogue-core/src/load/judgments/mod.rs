//! Checks for judgment files.

mod approval;
mod check;
mod references;
mod supersedes;

pub(super) use check::{Refs, check_judgments};
