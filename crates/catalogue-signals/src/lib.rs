//! Raw catalogue signals collected from external sources.
//!
//! [`http`] holds the shared HTTP transport. [`fetch`] downloads the crates.io
//! database dump. [`dump`] reads it. [`usage`] derives adoption signals from
//! what it holds. [`discover`] selects candidate crates from it. [`github`]
//! searches GitHub for candidate repos.

mod agent;
pub mod discover;
pub mod dump;
pub mod fetch;
pub mod github;
pub mod http;
pub mod usage;
