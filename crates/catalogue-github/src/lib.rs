//! The GitHub App client of the ml-rust catalogue bot.
//!
//! [`AppKey`] reads the App private key and refuses a file that group or
//! other can read. [`GithubApp`] signs short-lived App JWTs and mints
//! installation tokens. [`RepoClient`] calls the REST API for one repo with an
//! installation token, which it refreshes before it runs out. Every request
//! goes through a [`Transport`](catalogue_signals::http::Transport) and a
//! [`Pacer`](catalogue_signals::http::Pacer), so tests run without a network.
//!
//! The key, the JWT and the token never appear in `Debug` output, errors or
//! files. Requests send the catalogue `User-Agent` and no personal identifier.

mod api;
mod app;
mod client;
mod error;
mod labels;
mod secret;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use api::{API_URL, MAX_PAGES, WRITE_INTERVAL};
pub use app::{Access, AppKey, InstallationToken, KeyError, Permissions, REFRESH_MARGIN};
pub use client::{Clock, GithubApp, RepoClient};
pub use error::ApiError;
pub use labels::{Label, LabelFields};
