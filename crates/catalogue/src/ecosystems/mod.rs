//! The `catalogue ecosystems` command: dependent counts from ecosyste.ms.

mod command;
mod render;

pub use command::{EcosystemsCommand, EcosystemsCommandError, run_ecosystems};
pub use render::{render_all, render_crate};
