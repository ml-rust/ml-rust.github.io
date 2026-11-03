//! Percentiles of the tier signals, for setting the thresholds in
//! `data/thresholds.toml`.
//!
//! [`percentiles`] reads nearest-rank percentiles from a list of samples.
//! [`Distribution`] holds the percentiles of one named signal.
//! [`crate_samples`] collects the crate signals of a population and
//! [`app_samples`] the release downloads of app repos.

mod apps;
mod crates;
mod distribution;
mod percentile;

pub use apps::app_samples;
pub use crates::{CrateSamples, crate_samples};
pub use distribution::{Distribution, STANDARD_POINTS};
pub use percentile::percentiles;
