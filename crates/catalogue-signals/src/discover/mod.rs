//! Candidate discovery over a crates.io database dump snapshot.
//!
//! [`candidates`] selects crates by category, keyword or dependency on a core
//! crate, and includes the core crates. The selection is a pure function of the snapshot and the [`Rules`].

mod candidates;
mod core_crates;
mod types;

pub use candidates::candidates;
pub use types::{Candidate, Discovered, Reason, Rules};
