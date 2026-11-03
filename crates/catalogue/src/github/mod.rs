//! The `catalogue github` command: facts about GitHub repos.

mod command;
mod render;

pub use command::{GithubCommand, GithubCommandError, run_github};
pub use render::{render_all, render_facts};
