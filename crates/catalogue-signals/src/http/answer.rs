//! What a paced request gives: an answer with its body, or an error.

use std::io::Read;

use thiserror::Error;

use super::error::HttpError;
use super::headers::Headers;

/// A paced request that ends without a usable body.
#[derive(Debug, Error)]
pub enum PacedError {
    /// The HTTP layer fails before the server answers.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The server answers with an unexpected status.
    #[error("{url} answered with status {status}")]
    Status {
        /// The requested URL.
        url: String,
        /// The HTTP status code.
        status: u16,
    },
    /// The response body breaks off.
    #[error("reading the body from {url} failed: {source}")]
    Body {
        /// The requested URL.
        url: String,
        /// The read error.
        source: std::io::Error,
    },
    /// The server keeps asking the caller to wait.
    #[error("{url} still answered with a rate limit after {retries} retries")]
    RetriesExhausted {
        /// The requested URL.
        url: String,
        /// The retries made after the first request.
        retries: u32,
    },
}

/// A response whose status the caller accepts, with its body read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The HTTP status code.
    pub status: u16,
    /// The response headers.
    pub headers: Headers,
    /// The body, up to the size cap.
    pub body: Vec<u8>,
}

/// Reads a response body up to `limit` bytes.
pub(super) fn read_body(url: &str, body: Box<dyn Read>, limit: u64) -> Result<Vec<u8>, PacedError> {
    let mut bytes = Vec::new();
    body.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|source| PacedError::Body {
            url: url.to_owned(),
            source,
        })?;
    Ok(bytes)
}
