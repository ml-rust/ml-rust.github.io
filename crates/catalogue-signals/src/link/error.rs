//! Errors from the link check.

use thiserror::Error;

use super::vcs_info::VcsInfoError;
use crate::github::GithubError;
use crate::http::HttpError;

/// A link check that ends without a result.
#[derive(Debug, Error)]
pub enum LinkError {
    /// The HTTP layer fails before the server answers.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The GitHub commit lookup fails.
    #[error(transparent)]
    Github(#[from] GithubError),
    /// The crate download answers with a status other than 200.
    #[error("{url} answered with status {status}. Check the crate name and version, then rerun")]
    Download {
        /// The requested URL.
        url: String,
        /// The HTTP status code.
        status: u16,
    },
    /// The `.crate` stream or its `.cargo_vcs_info.json` is unusable.
    #[error("cannot read the package {url}: {source}. Rerun later")]
    Package {
        /// The requested URL.
        url: String,
        /// What is wrong with the stream or the file.
        source: VcsInfoError,
    },
    /// A crate name or version holds a character that a URL or file name cannot carry.
    #[error(
        "`{value}` is not a usable crate name or version. Use ASCII letters, digits, `.`, `_`, `-` and `+`"
    )]
    UnsafeName {
        /// The rejected name or version.
        value: String,
    },
    /// A link cache file cannot be read or written.
    #[error("{}: {source}. Check the path and its permissions, then rerun", .path.display())]
    Cache {
        /// The cache path.
        path: std::path::PathBuf,
        /// The error from the operating system.
        source: std::io::Error,
    },
}
