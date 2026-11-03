//! The `catalogue discover` command: candidate crates from the crates.io dump
//! and candidate repos from GitHub.

mod command;
mod github;
mod report;

pub use command::{DiscoverCommand, DiscoverError, run_discover};
pub(crate) use command::{crate_rules, load_catalogue, report_error};
pub use github::{GithubReport, build_github_report};
pub(crate) use github::{leads, search};
pub use report::{Report, build_report};
