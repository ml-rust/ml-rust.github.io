//! The `catalogue seed` command: draft entries from discovery for human review.

mod build;
mod command;
mod error;
mod leads;
mod merge;
mod write;

pub use build::{Seed, build_seed};
pub(crate) use command::{DEFAULT_MIN_USERS, LEAD_FILE, parse_min_users};
pub use command::{SeedArgs, run_seed};
pub use error::SeedError;
pub use leads::{LeadSeed, build_lead_seed, parse_leads};
pub use merge::build_merged_seed;
pub use write::write_drafts;
