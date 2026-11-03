//! Data model, parsing and cross-file validation for the ml-rust catalogue.
//!
//! The crate reads no files and opens no connections. Callers pass the raw
//! text of each data file in a [`DataSources`] and receive a checked
//! [`Catalogue`] or the full list of [`Diagnostics`].

mod catalogue;
mod date;
pub mod deprecation;
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
pub use ids::{
    CrateName, GithubLogin, IdError, Slug, canonical_crate_name, normalize_repo_url,
    push_canonical_crate_name,
};
pub use judgment::{ISSUE_URL_PREFIX, Judgment, JudgmentKind, MaintainerStatus, ProjectKind};
pub use load::{DataSources, Diagnostic, Diagnostics, SourceFile};
pub use registry::{
    Discovery, Maintainer, Maintainers, SynonymGroup, Synonyms, Topic, TopicSummary, Topics,
};
pub use tier::Tier;
