//! Parsing and validation of a whole data set.

mod categories;
mod deprecated;
mod diagnostics;
mod discovery;
mod entries;
mod graph;
mod judgments;
mod maintainers;
mod mappings;
mod parse;
mod run;
mod sources;
mod synonyms;
mod thresholds;
mod topics;

pub use diagnostics::{Diagnostic, Diagnostics};
pub(crate) use judgments::{
    approval_problems, eligible_approvers, reference_problems, submission_approvers,
};
pub(crate) use run::load;
pub use sources::{DataPlace, DataSources, SourceFile};
