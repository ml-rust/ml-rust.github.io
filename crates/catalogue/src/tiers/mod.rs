//! The `catalogue tiers` command: every entry's tier with the reasons, as text
//! and as a JSON report with the source of each signal.

mod assess;
mod collect;
mod command;
mod docker;
mod error;
mod inputs;
mod links;
mod render;
mod report;
mod run;
mod step_summary;

pub use collect::Network;
pub use command::{TiersArgs, run_tiers};
pub use error::TiersError;
pub use inputs::TiersRun;
pub use run::tiers_report;

pub(crate) use collect::github_repo;
