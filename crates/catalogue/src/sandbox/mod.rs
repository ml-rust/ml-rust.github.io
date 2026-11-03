//! The `catalogue sandbox` commands: the weekly `cargo check` of dormant crates.

mod check;
mod command;
mod error;
mod files;
mod guard;
mod list;
mod probe;
mod run;

pub use command::{CheckArgs, ListArgs, SandboxCommand, run_sandbox};
pub use error::SandboxCommandError;
pub use files::read_build_checks;
pub use list::{SandboxList, sandbox_list};
