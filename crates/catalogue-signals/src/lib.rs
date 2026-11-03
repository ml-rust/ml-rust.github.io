//! Raw catalogue signals collected from external sources.
//!
//! [`http`] holds the shared HTTP transport. [`crates_io`] reads the owners
//! of one crate from the crates.io API. [`fetch`] downloads the crates.io
//! database dump and the RustSec advisory archive. [`dump`] reads the dump.
//! [`usage`] derives adoption signals from what it holds. [`discover`] selects
//! candidate crates from it. [`github`] searches GitHub for candidate repos.
//! [`ecosystems`] reads dependent counts from ecosyste.ms. [`distribution`]
//! reads release, Homebrew, Docker Hub and Repology counts for apps.
//! [`rustsec`] reads the advisory archive. [`seed`] drafts entries from the
//! candidates. [`link`] checks that a crate was published from its repo.
//! [`history`] keeps the daily snapshots that the two-check dead rule and
//! Docker growth read. [`assemble`] turns all of them into the tier signals of
//! one entry. [`calibrate`] reads percentiles of the tier signals, for setting
//! thresholds. [`repo_archive`] reads the data files from a repo archive.
//! [`atomic`] replaces a file so a reader never sees a partial one.

mod agent;
mod archive;
pub mod assemble;
pub mod atomic;
pub mod calibrate;
pub mod crates_io;
pub mod discover;
pub mod distribution;
pub mod dump;
pub mod ecosystems;
pub mod fetch;
pub mod github;
pub mod history;
pub mod http;
pub mod link;
pub mod repo_archive;
pub mod rustsec;
pub mod seed;
pub mod usage;

pub use agent::USER_AGENT;
