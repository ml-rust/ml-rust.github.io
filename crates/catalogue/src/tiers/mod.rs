//! The `catalogue tiers` command: every entry's tier with the reasons, as text
//! and as a JSON report with the source of each signal.

mod assess;
mod collect;
mod command;
mod docker;
mod error;
mod links;
mod render;
mod report;
mod step_summary;

pub use collect::Network;
pub use command::{TiersArgs, TiersRun, run_tiers, tiers_report};
pub use error::TiersError;

pub(crate) use collect::github_repo;
