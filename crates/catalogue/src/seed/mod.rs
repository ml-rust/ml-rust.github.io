//! The `catalogue seed` command: draft entries from discovery for human review.

mod build;
mod command;
mod error;
mod write;

pub use build::{Seed, build_seed};
pub use command::{SeedArgs, run_seed};
pub use error::SeedError;
pub use write::write_drafts;
