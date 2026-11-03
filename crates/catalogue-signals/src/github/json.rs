//! JSON parsing of GitHub response bodies.

use serde::de::DeserializeOwned;

use super::error::GithubError;

/// Parses a response body as JSON.
pub(super) fn parse_json<T: DeserializeOwned>(url: &str, bytes: &[u8]) -> Result<T, GithubError> {
    serde_json::from_slice(bytes).map_err(|source| GithubError::Json {
        url: url.to_owned(),
        source,
    })
}
