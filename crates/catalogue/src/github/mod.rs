//! The `catalogue github` command: facts about GitHub repos and the link between a crate and its repo.

mod command;
mod link;
mod render;

pub use command::{GithubCommand, GithubCommandError, run_github};
pub use link::render_link;
pub use render::{render_all, render_facts};
