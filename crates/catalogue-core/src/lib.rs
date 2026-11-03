//! Data model, parsing, cross-file validation and tier rules for the ml-rust catalogue.
//!
//! The crate reads no files and opens no connections. Callers pass the raw
//! text of each data file in a [`DataSources`] and receive a checked
//! [`Catalogue`] or the full list of [`Diagnostics`]. [`assess`] applies the
//! tier rules to one entry's [`Signals`] and [`Context`].

mod catalogue;
mod date;
pub mod deprecation;
mod entry;
mod ids;
mod intake;
mod judgment;
mod judgment_checks;
mod load;
mod registry;
mod tier;

#[cfg(test)]
mod testing;

pub use catalogue::Catalogue;
pub use date::{Date, DateError};
pub use entry::{
    CRATES_IO_PAGE, Description, DescriptionSource, Distribution, Entry, EntryKind,
    MAX_DESCRIPTION_CHARS,
};
pub use ids::{
    CrateName, GithubLogin, IdError, Slug, canonical_crate_name, normalize_repo_url,
    push_canonical_crate_name,
};
pub use intake::{description_text, entry_name, repo_link};
pub use judgment::{ISSUE_URL_PREFIX, Judgment, JudgmentKind, MaintainerStatus, ProjectKind};
pub use load::{DataSources, Diagnostic, Diagnostics, SourceFile};
pub use registry::{
    AppFeaturedThresholds, Deprecated, DeprecatedRequirement, Discovery, DormancyThresholds,
    FinishedThresholds, IntervalThresholds, LibraryFeaturedThresholds, Maintainer, Maintainers,
    NewThresholds, RequirementCheck, RotThresholds, SynonymGroup, Synonyms, Thresholds, Topic,
    TopicSummary, Topics,
};
pub use tier::{Context, Reason, Rule, Signals, Tier, Verdict, assess};
