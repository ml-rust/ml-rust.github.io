//! What a failed network fetch does to one `catalogue tiers` run.
//!
//! [`distribution_failure`], [`github_failure`] and [`link_failure`] sort every
//! fetch error into a [`Failure`]. [`recover`] picks up the value the previous
//! snapshot holds, inside the age limit. [`FetchSummary`] names each fallback.
//!
//! Only network fetches fall back. A missing dump or RustSec archive still ends
//! the run: those have their own files and no value in a snapshot.

mod classify;
mod recover;
mod source;
mod summary;

pub use classify::{Failed, Failure, attempt, distribution_failure, github_failure, link_failure};
pub use recover::{Recovered, recover};
pub use source::Source;
pub use summary::{FetchFailure, FetchSummary, MIN_FETCHES_FOR_FAILED_SHARE};
