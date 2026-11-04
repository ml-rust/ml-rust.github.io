//! The README of a repo, for display on its entry page. No tier rule reads it.
//!
//! [`github_readme`] fetches it through the GitHub REST API. [`forge_readme`] reads the root
//! folder of a [`ForgeRepo`](super::ForgeRepo) and fetches the first README file it holds.

mod base64;
mod forge;
mod github;
mod text;

pub use forge::forge_readme;
pub use github::github_readme;
pub use text::{MAX_README, RepoReadme};
