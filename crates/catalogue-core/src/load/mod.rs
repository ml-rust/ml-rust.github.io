//! Parsing and validation of a whole data set.

mod diagnostics;
mod discovery;
mod entries;
mod graph;
mod judgments;
mod maintainers;
mod parse;
mod run;
mod sources;
mod synonyms;
mod topics;

pub use diagnostics::{Diagnostic, Diagnostics};
pub(crate) use run::load;
pub use sources::{DataSources, SourceFile};
