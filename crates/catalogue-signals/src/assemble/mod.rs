//! Per-entry signal assembly: turns the dump, usage, advisories, repo facts
//! and distribution counts into the [`Signals`](catalogue_core::Signals) of
//! one entry, and judgments into its [`Context`](catalogue_core::Context).
//!
//! Every function is pure. [`library_signals`] reads the dump through a
//! [`CrateIndex`] built once per check.

mod adoption;
mod app;
mod assembled;
mod build;
mod context;
mod facts;
#[cfg(test)]
mod fixtures;
mod index;
mod library;
mod link;
mod requirements;
mod sources;
mod versions;

pub use app::app_signals;
pub use assembled::Assembled;
pub use context::context;
pub use index::CrateIndex;
pub use library::library_signals;
pub use link::{latest_release, primary_release};
pub use sources::LibrarySources;
