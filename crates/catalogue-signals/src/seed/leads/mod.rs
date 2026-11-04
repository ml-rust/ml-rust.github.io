//! Draft entries from a list of GitHub repos.
//!
//! [`seed_leads`] resolves each lead with the GitHub API, then applies the family and topic
//! rules of the dump flow. A lead bypasses the discovery rules and the active-user floor,
//! because tier rules handle small projects.

mod channels;
mod claims;
mod decide;
mod entry;
mod kind;
mod languages;
mod resolve;
mod run;
mod types;

pub use run::seed_leads;
pub use types::{DraftDetail, LeadDrafts, LeadInputs, LeadPacers};
