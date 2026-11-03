//! The `catalogue rustsec` command: fetch and show.

mod command;
mod error;
mod render;

pub use command::{RustsecCommand, run_rustsec};
pub use error::RustsecCommandError;
pub use render::render_crate;
