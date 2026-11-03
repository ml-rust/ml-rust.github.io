//! ecosyste.ms signals: how many packages and repos depend on a crate.
//!
//! [`crate_dependents`] reads the dependent counts of one crates.io package from
//! the packages.ecosyste.ms API. An [`IntervalPacer`](crate::http::IntervalPacer)
//! set to [`ECOSYSTEMS_INTERVAL`] keeps the requests inside the anonymous rate
//! limit. The HTTP layer sits behind
//! [`Transport`](crate::http::Transport), so the lookup runs without a network.
//!
//! The repos.ecosyste.ms API exposes no dependent count for a repo. Its usage
//! endpoint counts dependents per package, so no repo-level function exists.

mod dependents;
mod error;
mod pacer;
mod types;

pub use dependents::crate_dependents;
pub use error::EcosystemsError;
pub use pacer::ECOSYSTEMS_INTERVAL;
pub use types::CrateDependents;
