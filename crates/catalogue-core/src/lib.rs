//! Data model, parsing, cross-file validation and tier rules for the ml-rust catalogue.
//!
//! The crate reads no files and opens no connections. Callers pass the raw
//! text of each data file in a [`DataSources`] and receive a checked
//! [`Catalogue`] or the full list of [`Diagnostics`]. [`assess`] applies the
//! tier rules to one entry's [`Signals`] and [`Context`]. A [`TierReport`]
//! holds every entry's tier with the source of each signal.
//! [`Catalogue::effective`] applies an entry's merged judgments.

pub mod amount_text;
mod catalogue;
mod date;
pub mod date_text;
pub mod deprecation;
mod effective;
mod entry;
mod ids;
mod intake;
mod judgment;
mod judgment_checks;
mod load;
mod project_pages;
mod registry;
mod report;
mod tier;

#[cfg(test)]
mod testing;

pub use catalogue::Catalogue;
pub use date::{Date, DateError};
pub use effective::{Correction, EffectiveEntry};
pub use entry::{
    CRATES_IO_PAGE, Description, DescriptionSource, Distribution, Entry, EntryKind,
    MAX_DESCRIPTION_CHARS,
};
pub use ids::{
    CommitSha, CrateName, GithubLogin, IdError, Slug, canonical_crate_name, is_github_repo_name,
    normalize_repo_url, push_canonical_crate_name,
};
pub use intake::{description_text, entry_name, repo_link};
pub use judgment::{ISSUE_URL_PREFIX, Judgment, JudgmentKind, MaintainerStatus, ProjectKind};
pub use load::{DataPlace, DataSources, Diagnostic, Diagnostics, SourceFile};
pub use project_pages::{crate_page, docker_page, homebrew_page, page_url, repology_page};
pub use registry::{
    AppFeaturedThresholds, Deprecated, DeprecatedRequirement, Discovery, DormancyThresholds,
    FallbackThresholds, FinishedThresholds, IntervalThresholds, LibraryFeaturedThresholds,
    Maintainer, Maintainers, NewThresholds, RequirementCheck, RotThresholds, SynonymGroup,
    Synonyms, Thresholds, Topic, TopicSummary, Topics,
};
pub use report::{
    EntryReport, REPORT_SCHEMA, ReasonRecord, ReportMismatch, STARS_SIGNAL, SignalFact, SourceRef,
    TierReport,
};
pub use tier::{Context, Reason, Rule, Signals, Tier, Verdict, assess};
