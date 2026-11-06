//! The state report: one snapshot of ML in Rust as of the day it was written.
//!
//! No tier reads this file. Models write the words and pick the charts. Code draws the charts.

use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::date_text::plain;
use crate::ids::Slug;

/// The version of the state report format. A change to any field raises it.
pub const STATE_SCHEMA: u32 = 1;

/// One state report. A new report replaces the last one, so the site shows only the current state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateReport {
    /// The report format version, [`STATE_SCHEMA`] when written.
    pub schema: u32,
    /// The date the models wrote the report.
    #[serde(serialize_with = "plain")]
    pub written: Date,
    /// What the models wrote.
    pub summary: StateSummary,
    /// The charts the models picked, in page order.
    pub charts: Vec<ChartNote>,
}

/// The words of the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSummary {
    /// One line that answers whether a team can build ML in Rust today.
    pub headline: String,
    /// A short paragraph behind the headline, including where Python is still needed.
    pub summary: String,
    /// How each job stands, one row per topic.
    pub jobs: Vec<JobState>,
    /// The crates to use now.
    pub use_now: Vec<ToolPick>,
    /// The young crates to watch.
    pub watch: Vec<ToolPick>,
    /// What a team can build today, and with which projects.
    pub build_today: Vec<BuildPath>,
    /// The needs Rust now covers.
    pub holes_filled: Vec<FilledHole>,
    /// The gaps that still stop real work.
    pub holes_open: Vec<OpenHole>,
}

/// How ready one job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Readiness {
    /// A team can ship this today and not regret it.
    Ready,
    /// A team can ship this, with a limit it must plan around.
    Usable,
    /// A team cannot rely on Rust for this yet.
    Early,
}

/// How much of one job still needs Python.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PythonNeed {
    /// The whole job works with no Python.
    None,
    /// Some steps of the job still need Python.
    Partly,
    /// The job still lives in Python.
    Needed,
}

/// One job: how it stands, and the tools to follow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobState {
    /// The topic id.
    pub topic: Slug,
    /// How ready the job is.
    pub readiness: Readiness,
    /// How much of the job still needs Python.
    pub python: PythonNeed,
    /// One or two sentences: what works and where the limit is.
    pub verdict: String,
    /// The key tools of the job.
    pub tools: Vec<Slug>,
}

/// One crate worth a reader's time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPick {
    /// The project.
    pub project: Slug,
    /// What to use it for.
    pub use_for: String,
    /// Why it earns its place.
    pub why: String,
}

/// One thing to build and what to build it with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildPath {
    /// What a team wants to build.
    pub goal: String,
    /// The projects to build it with.
    pub stack: Vec<Slug>,
    /// Why this stack works.
    pub why: String,
    /// The limit to plan around.
    pub watch_out: String,
}

/// One need Rust now covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilledHole {
    /// The need.
    pub hole: String,
    /// The projects that cover it.
    pub filled_by: Vec<Slug>,
    /// What this means for a team.
    pub note: String,
}

/// One missing ability that stops real work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenHole {
    /// What is missing.
    pub hole: String,
    /// Who it stops, and from doing what.
    pub impact: String,
    /// What teams do today instead.
    pub workaround: String,
}

/// The measured charts of the state page. Code draws each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartId {
    /// Whether the work happens in Rust or in wrapped native code.
    Engine,
    /// How recently projects shipped.
    Releases,
    /// The GPU backends the check can see.
    Gpu,
    /// How many people stand behind projects.
    Team,
    /// How many other teams build on projects.
    UsedBy,
}

/// One picked chart and what it shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartNote {
    /// The chart.
    pub chart: ChartId,
    /// One sentence on what a reader takes from it.
    pub takeaway: String,
}
