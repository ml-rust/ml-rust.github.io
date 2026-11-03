//! The `catalogue discover` command: candidate crates from the crates.io dump.

mod command;
mod report;

pub use command::{DiscoverCommand, DiscoverError, run_discover};
pub use report::{Report, build_report};
