//! Errors from the distribution lookups.

use thiserror::Error;

use super::releases::API as RELEASES_API;
use crate::http::{HttpError, PacedError};

/// A lookup the fetcher cannot finish.
///
/// Every variant except [`Http`](Self::Http) names the API in `api`.
#[derive(Debug, Error)]
pub enum DistributionError {
    /// The HTTP layer fails before the server answers.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The server answers with a status other than 200 and 404.
    #[error("{api}: {url} answered with status {status}. {action}")]
    Status {
        /// The API name.
        api: &'static str,
        /// The requested URL.
        url: String,
        /// The HTTP status code.
        status: u16,
        /// The next action for the caller.
        action: &'static str,
    },
    /// The response body breaks off.
    #[error("{api}: reading the body from {url} failed: {source}. Rerun later")]
    Body {
        /// The API name.
        api: &'static str,
        /// The requested URL.
        url: String,
        /// The read error.
        source: std::io::Error,
    },
    /// The response body is not the expected JSON.
    #[error(
        "{api}: the response from {url} is not the expected JSON: {source}. Rerun later. If the error persists, report it"
    )]
    Json {
        /// The API name.
        api: &'static str,
        /// The requested URL.
        url: String,
        /// The parse error.
        source: serde_json::Error,
    },
    /// A field holds a value the lookup cannot read.
    #[error(
        "{api}: the `{field}` field from {url} holds `{value}`, which is not a timestamp. Rerun later. If the error persists, report it"
    )]
    BadField {
        /// The API name.
        api: &'static str,
        /// The requested URL.
        url: String,
        /// The field name.
        field: &'static str,
        /// The rejected value.
        value: String,
    },
    /// The server keeps asking the caller to wait.
    #[error("{api}: {url} still answered with a rate limit after {retries} retries. Rerun later")]
    RetriesExhausted {
        /// The API name.
        api: &'static str,
        /// The requested URL.
        url: String,
        /// The retries made after the first request.
        retries: u32,
    },
}

impl DistributionError {
    /// The error for a paced request to the API `api` that ends without a body.
    ///
    /// A [`PacedError`] does not name the API, so no plain `From` exists.
    pub(super) fn paced(api: &'static str, error: PacedError) -> Self {
        match error {
            PacedError::Http(error) => Self::Http(error),
            PacedError::Status { url, status } => Self::status(api, &url, status),
            PacedError::Body { url, source } => Self::Body { api, url, source },
            PacedError::RetriesExhausted { url, retries } => {
                Self::RetriesExhausted { api, url, retries }
            }
        }
    }

    /// The error for a response with a status other than 200 and 404.
    fn status(api: &'static str, url: &str, status: u16) -> Self {
        let action = match status {
            401 if api == RELEASES_API => "set GITHUB_TOKEN to a valid token",
            403 | 429 => "wait for the rate limit to reset, then rerun",
            _ => "rerun later",
        };
        Self::Status {
            api,
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
    fn a_status_error_names_the_api_the_url_and_an_action() {
        let message = DistributionError::status("Repology", "https://x/y", 429).to_string();
        assert_eq!(
            message,
            "Repology: https://x/y answered with status 429. wait for the rate limit to reset, then rerun"
        );
    }

    #[test]
    fn an_unauthorized_status_names_the_token() {
        let message = DistributionError::status("GitHub Releases", "u", 401).to_string();
        assert!(
            message.ends_with("set GITHUB_TOKEN to a valid token"),
            "{message}"
        );
    }

    #[test]
    fn a_401_from_an_api_with_no_token_does_not_name_the_token() {
        let message = DistributionError::status("Docker Hub", "u", 401).to_string();
        assert!(message.ends_with("status 401. rerun later"), "{message}");
    }

    #[test]
    fn other_statuses_say_to_rerun() {
        let message = DistributionError::status("Docker Hub", "u", 500).to_string();
        assert!(message.ends_with("status 500. rerun later"), "{message}");
    }

    #[test]
    fn a_paced_error_keeps_the_url_and_gains_the_api() {
        let error = DistributionError::paced(
            "Homebrew",
            PacedError::RetriesExhausted {
                url: "u".to_owned(),
                retries: 3,
            },
        );
        assert!(
            matches!(
                error,
                DistributionError::RetriesExhausted {
                    api: "Homebrew",
                    retries: 3,
                    ..
                }
            ),
            "{error}"
        );
    }
}
