//! Draft entries from discovery candidates.
//!
//! [`draft_entries`] turns each eligible repository into one entry for human
//! review. The result is a pure function of the dump, the usage signals, the
//! topic and category mappings and the entries that already exist. Eligible
//! crates that name the same `host/owner/repo` and share owners form one
//! family, and a family yields one draft that lists all its crates, the main
//! crate first.
//!
//! [`seed_leads`] drafts library and app entries from a list of GitHub repos. It
//! bypasses the discovery rules and the active-user floor.

mod draft;
mod family;
mod family_draft;
mod fields;
#[cfg(test)]
mod fixtures;
mod leads;
mod names;
mod owners;
mod primary;
mod related;
mod types;

pub use draft::draft_entries;
pub use leads::{DraftDetail, LeadDrafts, LeadInputs, LeadPacers, seed_leads};
pub(crate) use primary::RepoName;
pub use types::{Drafts, ExistingIndex, Mappings};
