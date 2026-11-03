//! The `catalogue dump` command: fetch, stats and usage.

mod command;
mod error;
mod render;

pub(crate) use command::read_cached;
pub use command::{DumpCommand, run_dump};
pub use error::DumpCommandError;
pub use render::{render_outcome, render_stats};
