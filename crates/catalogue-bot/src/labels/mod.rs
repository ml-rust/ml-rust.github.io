//! The repo labels file and the plan that syncs GitHub to it.

mod file;
mod plan;

pub use file::{DESCRIPTION_MAX, LabelProblem, LabelSpec, LabelsError, LabelsFile, parse_labels};
pub use plan::{LabelUpdate, RepoLabel, SyncPlan, plan_sync};
