//! Errors from downloading the crates.io database dump.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// A download the fetcher cannot finish.
#[derive(Debug, Error)]
pub enum FetchError {
    /// The HTTP client cannot be built.
    #[error(
        "cannot build the HTTP client: {reason}. Check the TLS and proxy environment and retry"
    )]
    Client {
        /// What the client builder reports.
        reason: String,
    },
    /// A request fails before the server answers.
    #[error("request to {url} failed: {source}. Check the network and rerun to retry or resume")]
    Request {
        /// The requested URL.
        url: String,
        /// The transport error.
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// The server answers with a status the fetcher does not handle.
    #[error(
        "{url} answered with status {status}, expected 200, 206 or 304. Rerun later. If the error persists, remove the `dump` directory under the cache directory"
    )]
    Status {
        /// The requested URL.
        url: String,
        /// The HTTP status code.
        status: u16,
    },
    /// The response body breaks off.
    #[error("reading the body from {url} failed: {source}. Rerun to resume the download")]
    Body {
        /// The requested URL.
        url: String,
        /// The read error.
        source: std::io::Error,
    },
    /// A file or directory operation fails.
    #[error("{}: {source}. Check the path and its permissions, then rerun", .path.display())]
    Io {
        /// The file or directory that fails.
        path: PathBuf,
        /// The error from the operating system.
        source: std::io::Error,
    },
    /// The body ends before the announced size.
    #[error(
        "{} holds {received} bytes, but the server announced {expected}. Rerun to resume the download",
        .path.display()
    )]
    Incomplete {
        /// The partial file.
        path: PathBuf,
        /// The bytes on disk.
        received: u64,
        /// The bytes the server announced.
        expected: u64,
    },
    /// A `206` response has a missing or unexpected `Content-Range`.
    #[error(
        "{url} answered 206 with `Content-Range: {value}`, expected a range that starts at byte {offset}. Remove the `.partial` file under the cache directory and rerun"
    )]
    ContentRange {
        /// The requested URL.
        url: String,
        /// The header value, or `<missing>`.
        value: String,
        /// The byte offset the request asked for.
        offset: u64,
    },
    /// `state.json` is not valid.
    #[error("{} is not valid: {source}. Remove the file and rerun", .path.display())]
    State {
        /// The state file.
        path: PathBuf,
        /// The JSON error.
        source: serde_json::Error,
    },
}

impl FetchError {
    /// Wraps an OS error as [`FetchError::Io`] for `path`.
    pub(super) fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::Io { path, source }
    }
}
