//! Raw catalogue signals collected from external sources.
//!
//! [`http`] holds the shared HTTP transport. [`fetch`] downloads the crates.io
//! database dump and the RustSec advisory archive. [`dump`] reads the dump.
//! [`usage`] derives adoption signals from what it holds. [`discover`] selects
//! candidate crates from it. [`github`] searches GitHub for candidate repos.
//! [`rustsec`] reads the advisory archive.

mod agent;
mod archive;
pub mod discover;
pub mod dump;
pub mod fetch;
pub mod github;
pub mod http;
pub mod rustsec;
pub mod usage;
