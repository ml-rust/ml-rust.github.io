//! The state report of the ml-rust catalogue: one snapshot of ML in Rust as of today.
//!
//! [`Projects`] turns the catalogue file into what a model reads: what each project does and how
//! real it is, with no stars, downloads or tiers. [`write_report`] runs steps with one purpose
//! each: shortlist the few READMEs still worth reading, read every project into notes, write the report, pick the charts, combine them in code,
//! audit it, and fix what the audit finds. Every step tries the models in priority order, and a
//! later model runs only when the ones before it give no valid answer. Every answer passes the checks in
//! [`parse_summary`] and [`parse_charts`], or it never reaches the report.
//!
//! No tier reads the report.

mod check;
mod facts;
mod needs;
mod prompt;
mod readme;
mod summary;
mod write;

pub use check::{parse_charts, parse_notes, parse_review, parse_shortlist, parse_summary};
pub use facts::{FactsError, Project, Projects};
pub use needs::{Need, NeedList, TopicNeeds, need_list};
pub use readme::{MAX_CHANGELOG, MAX_README, changelog_text, readme_text, release_readme_text};
pub use summary::{
    RefreshReport, ReleaseSources, SummaryStore, parse_project_summary, refresh_summaries,
};
pub use write::{MAX_READMES, WriteError, write_report};
