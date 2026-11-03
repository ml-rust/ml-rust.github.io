//! The `catalogue build` command: the static site from the data files.

mod error;
mod publish;
mod run;

pub use error::BuildError;
pub use run::{BuildArgs, BuildSummary, build, run_build};
