//! The `catalogue discover` command: candidate crates from the crates.io dump
//! and candidate repos from GitHub.

mod command;
mod github;
mod report;

pub use command::{DiscoverCommand, DiscoverError, run_discover};
pub use github::{GithubReport, build_github_report};
pub use report::{Report, build_report};
