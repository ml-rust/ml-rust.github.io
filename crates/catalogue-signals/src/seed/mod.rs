//! Draft entries from discovery candidates.
//!
//! [`draft_entries`] turns each eligible repository into one entry for human
//! review. The result is a pure function of the dump, the usage signals, the
//! topic mappings and the entries that already exist. Eligible crates whose
//! repository links normalise to the same `host/owner/repo` form one family,
//! and a family yields one draft that lists all its crates.

mod draft;
mod family;
mod family_draft;
mod fields;
#[cfg(test)]
mod fixtures;
mod names;
mod types;

pub use draft::draft_entries;
pub use types::{Drafts, ExistingIndex};
