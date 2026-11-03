//! The repo link check of a library entry.
//!
//! A library's repo link holds when the primary crate's latest unyanked
//! version was published from a commit that the default branch of the entry's
//! GitHub repo contains. A commit that only a fork holds does not count.
//! [`read_vcs_info`] reads `.cargo_vcs_info.json` from a `.crate` stream.
//! [`commit_standing`] asks GitHub where the commit stands. [`check_link`]
//! runs both for one crate release and keeps the result in the cache
//! directory. The HTTP layer sits behind [`Transport`](crate::http::Transport),
//! so the check runs without a network.

mod cache;
mod check;
mod error;
mod types;
mod vcs_info;
mod verify;

pub use cache::{cached_check, links_dir, store_check};
pub use check::{CommitStanding, commit_standing};
pub use error::LinkError;
pub use types::{CrateRelease, LinkCheck, LinkOutcome};
pub use vcs_info::{MAX_VCS_INFO, VcsInfo, VcsInfoError, read_vcs_info};
pub use verify::{cached_link, check_link};
