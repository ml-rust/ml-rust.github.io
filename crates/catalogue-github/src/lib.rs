//! The GitHub App client of the ml-rust catalogue bot.
//!
//! [`AppKey`] reads the App private key and refuses a file that group or
//! other can read. [`GithubApp`] signs short-lived App JWTs and mints
//! installation tokens. [`RepoClient`] calls the REST API for one repo with an
//! installation token, which it refreshes before it runs out. It lists issues
//! and comments, writes comments and issue labels, reads the data files of a
//! commit and the newest tier snapshot on a branch. It reads issue events
//! and PRs, builds one commit on a bot branch and opens the data PR. It reads
//! the owner of a repo and public org membership. It reads the README, an
//! issue or PR and a file of another public repo. Every request
//! goes through a [`Transport`](catalogue_signals::http::Transport) and a
//! [`Pacer`](catalogue_signals::http::Pacer), so tests run without a network.
//!
//! The key, the JWT and the token never appear in `Debug` output, errors or
//! files. [`Secret`] holds each of them. Requests send the catalogue
//! `User-Agent` and no personal identifier.

mod accounts;
mod api;
mod app;
mod client;
mod contents;
mod error;
mod issues;
mod labels;
mod pulls;
mod secret;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use api::{API_URL, MAX_PAGES, WRITE_INTERVAL};
pub use app::{Access, AppKey, InstallationToken, KeyError, Permissions, REFRESH_MARGIN};
pub use client::{Clock, GithubApp, RepoClient};
pub use contents::{BranchSnapshot, CommitData, MAX_ARCHIVE_BYTES};
pub use error::ApiError;
pub use issues::{Author, Comment, CommentUpdate, Issue, IssueEvent};
pub use labels::{Label, LabelFields};
pub use pulls::{BranchWrite, NewPull, Pull};
pub use secret::Secret;
