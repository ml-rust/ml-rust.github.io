//! Raw catalogue signals collected from external sources.
//!
//! [`fetch`] downloads the crates.io database dump. [`dump`] reads it.

mod agent;
pub mod dump;
pub mod fetch;
