//! Errors from the crates.io API lookups.

use thiserror::Error;

use crate::http::{HttpError, PacedError};

/// A lookup the client cannot finish.
#[derive(Debug, Error)]
pub enum CratesIoError {
    /// The HTTP layer fails before the server answers.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The server answers with a status other than 200 and 404.
    #[error("{url} answered with status {status}. {action}")]
    Status {
        /// The requested URL.
        url: String,
        /// The HTTP status code.
        status: u16,
        /// The next action for the caller.
        action: &'static str,
    },
    /// The response body breaks off.
    #[error("reading the body from {url} failed: {source}. Rerun later")]
    Body {
        /// The requested URL.
        url: String,
        /// The read error.
        source: std::io::Error,
    },
    /// The response body is not the expected JSON.
    #[error(
        "the response from {url} is not the expected JSON: {source}. Rerun later. If the error persists, report it"
    )]
    Json {
        /// The requested URL.
        url: String,
        /// The parse error.
        source: serde_json::Error,
    },
    /// The server keeps asking the caller to wait.
    #[error("{url} still answered with a rate limit after {retries} retries. Rerun later")]
    RetriesExhausted {
        /// The requested URL.
        url: String,
        /// The retries made after the first request.
        retries: u32,
    },
}

impl From<PacedError> for CratesIoError {
    fn from(error: PacedError) -> Self {
        match error {
            PacedError::Http(error) => Self::Http(error),
            PacedError::Status { url, status } => Self::Status {
                url,
                status,
                action: action(status),
            },
            PacedError::Body { url, source } => Self::Body { url, source },
            PacedError::RetriesExhausted { url, retries } => {
                Self::RetriesExhausted { url, retries }
            }
        }
    }
}

/// The next action for an answer with `status`.
fn action(status: u16) -> &'static str {
    match status {
        403 | 429 => "Wait for the rate limit to reset, then rerun",
        _ => "Rerun later",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_error_names_the_url_and_the_next_action() {
        let error = CratesIoError::from(PacedError::Status {
            url: "https://crates.io/api/v1/crates/x/owners".to_owned(),
            status: 429,
        });
        assert_eq!(
            error.to_string(),
            "https://crates.io/api/v1/crates/x/owners answered with status 429. Wait for the rate limit to reset, then rerun"
        );
    }
}
