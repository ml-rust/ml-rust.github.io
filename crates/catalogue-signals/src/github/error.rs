//! Errors from the GitHub search and the repo facts.

use thiserror::Error;

use crate::http::HttpError;

/// A search or facts query the fetcher cannot finish.
#[derive(Debug, Error)]
pub enum GithubError {
    /// The HTTP layer fails before the server answers.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The server answers with a status other than 200.
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
    /// The GraphQL API answers a query with an error.
    #[error(
        "GitHub rejected the query for {repo}: {message}. Check the token and the repo name, then rerun"
    )]
    Graphql {
        /// The requested `owner/name`.
        repo: String,
        /// The messages of the API errors, joined with `; `.
        message: String,
    },
    /// A date range cannot be split.
    #[error("cannot split the created range {from}..{to}: {reason}")]
    DateRange {
        /// The first day of the range.
        from: String,
        /// The last day of the range.
        to: String,
        /// What the date library reports.
        reason: String,
    },
}

impl GithubError {
    /// The error for a response with a status other than 200.
    pub(super) fn status(url: &str, status: u16) -> Self {
        let action = match status {
            401 => "set GITHUB_TOKEN to a valid token",
            403 | 429 => {
                "check that the token can read public repos, wait for the rate limit to reset, then rerun"
            }
            422 => "check github_topics in discovery.toml, then rerun",
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
    fn unauthorized_names_the_token() {
        let message = GithubError::status("https://api.github.com/x", 401).to_string();
        assert_eq!(
            message,
            "https://api.github.com/x answered with status 401. set GITHUB_TOKEN to a valid token"
        );
    }

    #[test]
    fn other_statuses_name_a_next_action() {
        for status in [403, 429, 422, 500] {
            let message = GithubError::status("u", status).to_string();
            assert!(message.contains(&status.to_string()), "{message}");
            assert!(
                message.ends_with("rerun") || message.ends_with("rerun later"),
                "{message}"
            );
        }
    }
}
