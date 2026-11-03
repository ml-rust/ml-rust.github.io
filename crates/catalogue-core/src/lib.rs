//! Data model, parsing and cross-file validation for the ml-rust catalogue.
//!
//! The crate reads no files and opens no connections. Callers pass the raw
//! text of each data file in a [`DataSources`] and receive a checked
//! [`Catalogue`] or the full list of [`Diagnostics`].

mod catalogue;
mod date;
mod entry;
mod ids;
mod judgment;
mod load;
mod registry;
mod tier;

#[cfg(test)]
mod testing;

pub use catalogue::Catalogue;
pub use date::{Date, DateError};
pub use entry::{Description, DescriptionSource, Distribution, Entry, EntryKind};
pub use ids::{CrateName, GithubLogin, IdError, Slug};
pub use judgment::{ISSUE_URL_PREFIX, Judgment, JudgmentKind, MaintainerStatus, ProjectKind};
pub use load::{DataSources, Diagnostic, Diagnostics, SourceFile};
pub use registry::{Maintainer, Maintainers, SynonymGroup, Synonyms, Topic, TopicSummary, Topics};
pub use tier::Tier;
