//! The `catalogue dump` command: fetch, stats and usage.

mod command;
mod error;
mod render;

pub use command::{DumpCommand, run_dump};
pub(crate) use command::{read_cached, read_cached_with_details};
pub use error::DumpCommandError;
pub use render::{render_outcome, render_stats};
