//! Errors from the GitHub App client.

use catalogue_signals::history::HistoryError;
use catalogue_signals::http::{HttpError, PacedError};
use catalogue_signals::repo_archive::RepoArchiveError;
use thiserror::Error;

/// A GitHub API call that ends without a usable answer.
///
/// No variant holds the key, the JWT or the token.
#[derive(Debug, Error)]
pub enum ApiError {
    /// The HTTP layer fails before the server answers.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The server answers with a status the call does not accept.
    #[error("{method} {url} answered with status {status}. {action}")]
    Status {
        /// The request method.
        method: &'static str,
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
    /// The server keeps asking the caller to wait.
    #[error("{url} still answered with a rate limit after {retries} retries. Rerun later")]
    RetriesExhausted {
        /// The requested URL.
        url: String,
        /// The retries made after the first request.
        retries: u32,
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
    /// The token response lacks a field or holds a bad one.
    ///
    /// The message names the field only. The body holds the token.
    #[error(
        "the installation token response from {url} has no valid `{field}`. Rerun later. If the error persists, report it"
    )]
    TokenResponse {
        /// The requested URL.
        url: String,
        /// The field that is missing or bad.
        field: &'static str,
    },
    /// A listing has more pages than the cap.
    #[error(
        "{url} lists more than {pages} pages. Nothing past the cap is read. Report it, so the cap can be checked"
    )]
    TooManyPages {
        /// The first page URL.
        url: String,
        /// The page cap.
        pages: usize,
    },
    /// A `Link` header points away from the GitHub API.
    #[error(
        "{url} links its next page to {link}, outside {api}. The token is not sent there. Report it"
    )]
    ForeignLink {
        /// The page URL that sent the link.
        url: String,
        /// The next page link.
        link: String,
        /// The API base URL.
        api: &'static str,
    },
    /// An answer arrives with a body that breaks a rule of the call.
    #[error("the answer from {url} is not usable: {reason}")]
    Content {
        /// The requested URL.
        url: String,
        /// What is wrong, with the next action.
        reason: String,
    },
    /// A repo archive cannot be read as the catalogue data.
    #[error("the repo archive from {url}: {source}")]
    Archive {
        /// The requested URL.
        url: String,
        /// What the archive reader reports.
        source: RepoArchiveError,
    },
    /// A fetched tier snapshot is corrupt.
    #[error("the tier snapshot from {url}: {source}")]
    Snapshot {
        /// The requested URL.
        url: String,
        /// What the snapshot reader reports.
        source: HistoryError,
    },
    /// The App JWT cannot be signed.
    #[error(
        "cannot sign the App JWT: {reason}. Check that the private key is the RSA key GitHub issued for the App"
    )]
    Sign {
        /// What the signer reports.
        reason: String,
    },
}

impl ApiError {
    /// Converts a [`PacedError`] for a `method` request.
    pub(crate) fn from_paced(method: &'static str, error: PacedError) -> Self {
        match error {
            PacedError::Http(error) => Self::Http(error),
            PacedError::Status { url, status } => Self::Status {
                method,
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
        401 => {
            "Check app_id, installation_id and the private key in the bot config, and that the system clock is right, then rerun"
        }
        403 => {
            "Check that the App holds the permission the call needs and is installed on the repo, then rerun"
        }
        404 => {
            "Check owner, repo and installation_id in the bot config, and that the App is installed on the repo, then rerun"
        }
        422 => "GitHub rejected the request body. Check the input the call sends, then rerun",
        429 => "Wait for the rate limit to reset, then rerun",
        _ => "Rerun later",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_error_names_the_call_and_the_next_action() {
        let error = ApiError::from_paced(
            "PATCH",
            PacedError::Status {
                url: "https://api.github.com/x".to_owned(),
                status: 404,
            },
        );
        let message = error.to_string();
        assert!(
            message.starts_with("PATCH https://api.github.com/x answered with status 404."),
            "{message}"
        );
        assert!(message.contains("installation_id"), "{message}");
    }

    #[test]
    fn every_status_has_an_action() {
        for status in [401, 403, 404, 422, 429, 500] {
            assert!(action(status).ends_with("rerun") || action(status).ends_with("later"));
        }
    }
}
