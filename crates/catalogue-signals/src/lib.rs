//! Raw catalogue signals collected from external sources.
//!
//! [`http`] holds the shared HTTP transport. [`fetch`] downloads the crates.io
//! database dump and the RustSec advisory archive. [`dump`] reads the dump.
//! [`usage`] derives adoption signals from what it holds. [`discover`] selects
//! candidate crates from it. [`github`] searches GitHub for candidate repos.
//! [`ecosystems`] reads dependent counts from ecosyste.ms. [`distribution`]
//! reads release, Homebrew, Docker Hub and Repology counts for apps.
//! [`rustsec`] reads the advisory archive. [`link`] checks that a crate was
//! published from its repo. [`assemble`] turns all of them into the tier
//! signals of one entry.

mod agent;
mod archive;
pub mod assemble;
pub mod discover;
pub mod distribution;
pub mod dump;
pub mod ecosystems;
pub mod fetch;
pub mod github;
pub mod http;
pub mod link;
pub mod rustsec;
pub mod usage;
