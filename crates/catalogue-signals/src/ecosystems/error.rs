//! Errors from the ecosyste.ms lookups.

use thiserror::Error;

use crate::http::{HttpError, PacedError};

/// A lookup the fetcher cannot finish.
#[derive(Debug, Error)]
pub enum EcosystemsError {
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

impl From<PacedError> for EcosystemsError {
    fn from(error: PacedError) -> Self {
        match error {
            PacedError::Http(error) => Self::Http(error),
            PacedError::Status { url, status } => Self::status(&url, status),
            PacedError::Body { url, source } => Self::Body { url, source },
            PacedError::RetriesExhausted { url, retries } => {
                Self::RetriesExhausted { url, retries }
            }
        }
    }
}

impl EcosystemsError {
    /// The error for a response with a status other than 200 and 404.
    fn status(url: &str, status: u16) -> Self {
        let action = match status {
            403 | 429 => "wait for the rate limit to reset, then rerun",
            _ => "rerun later",
        };
        Self::Status {
            url: url.to_owned(),
            status,
            action,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rate_limit_status_says_to_wait() {
        let message = EcosystemsError::status("https://x/y", 429).to_string();
        assert_eq!(
            message,
            "https://x/y answered with status 429. wait for the rate limit to reset, then rerun"
        );
    }

    #[test]
    fn other_statuses_say_to_rerun() {
        let message = EcosystemsError::status("u", 500).to_string();
        assert!(message.ends_with("status 500. rerun later"), "{message}");
    }
}
