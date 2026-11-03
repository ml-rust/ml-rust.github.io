//! Raw catalogue signals collected from external sources.
//!
//! [`fetch`] downloads the crates.io database dump. [`dump`] reads it.
//! [`usage`] derives adoption signals from what it holds. [`discover`] selects
//! candidate crates from it.

mod agent;
pub mod discover;
pub mod dump;
pub mod fetch;
pub mod usage;
