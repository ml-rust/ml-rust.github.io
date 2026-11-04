//! Per-entry signal assembly: turns the dump, usage, advisories, repo facts
//! and distribution counts into the [`Signals`](catalogue_core::Signals) of
//! one entry, and judgments into its [`Context`](catalogue_core::Context).
//!
//! Every function is pure. [`library_signals`] reads the dump through a
//! [`CrateIndex`] built once per check. [`signal_facts`] cites the source and
//! the data date of each signal for the entry page. [`choice`] measures the
//! facts a person reads to choose a project, and no tier rule reads them.

mod adoption;
mod app;
mod assembled;
mod build;
pub mod choice;
mod context;
mod facts;
#[cfg(test)]
mod fixtures;
mod index;
mod library;
mod link;
mod provenance;
mod requirements;
mod sources;
mod versions;

pub use app::app_signals;
pub use assembled::Assembled;
pub use context::context;
pub use facts::apply_contributors;
pub use index::CrateIndex;
pub use library::library_signals;
pub use link::{latest_release, primary_release};
pub use provenance::{
    ADVISORY_PAGE, BUILD_CHECK_PAGE, DUMP_PAGE, DUMP_SOURCE, FactSources, FetchDates,
    choice_signal_facts, signal_facts,
};
pub use sources::LibrarySources;
