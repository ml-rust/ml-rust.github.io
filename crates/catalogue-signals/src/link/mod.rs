//! The repo link check of a library entry.
//!
//! A library's repo link holds when the primary crate's latest unyanked
//! version was published from a commit that a branch or tag of the entry's
//! GitHub repo holds. A commit that only a fork holds does not count.
//! When the release records no commit, or the repo is on Codeberg or GitLab,
//! the link holds when a `Cargo.toml` on the repo's default branch declares
//! the crate. A release commit that the repo lacks stays a contradiction and
//! gets no manifest proof.
//! [`read_vcs_info`] reads `.cargo_vcs_info.json` from a `.crate` stream.
//! [`commit_standing`] asks GitHub where the commit stands. [`check_link`]
//! runs the checks for one crate release and keeps the result in the cache
//! directory. [`check_forge_link`] does the same for a [`ForgeRepo`]. The HTTP
//! layer sits behind [`Transport`](crate::http::Transport), so the check runs
//! without a network. [`github_readme`] and [`forge_readme`] fetch a repo's README
//! through the same hosts. No tier rule reads it.

mod cache;
mod check;
mod error;
mod forge;
mod forge_check;
mod manifest;
mod query;
mod readme;
#[cfg(test)]
mod test_host;
mod types;
mod vcs_info;
mod verify;

pub use cache::{cached_check, links_dir, store_check};
pub use check::{CommitStanding, commit_standing};
pub use error::LinkError;
pub use forge::{ForgeHost, ForgeRepo};
pub use forge_check::{cached_forge_link, check_forge_link};
pub use readme::{MAX_README, RepoReadme, forge_readme, github_readme};
pub use types::{CrateRelease, LinkCheck, LinkOutcome};
pub use vcs_info::{MAX_VCS_INFO, VcsInfo, VcsInfoError, read_vcs_info};
pub use verify::{cached_link, check_link};
