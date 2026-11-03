//! Page text: GitHub pages through the GitHub API, other pages through Ollama `web_fetch`.
//!
//! The bot host never sends a request to the host of an evidence link.

use std::time::Duration;

use catalogue_bot::triage::{GithubRepo, GithubTarget};
use catalogue_github::{ApiError, RepoClient, Secret};
use catalogue_signals::github::RepoRef;
use catalogue_signals::http::{Paced, PacedError, Pacer, Transport};
use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;
use url::Url;

use crate::headers::json_bearer;

/// The Ollama `web_fetch` endpoint.
pub const WEB_FETCH_URL: &str = "https://ollama.com/api/web_fetch";

/// The least time between two `web_fetch` requests.
pub const WEB_FETCH_INTERVAL: Duration = Duration::from_secs(1);

/// Why the text of one page is missing. Each message is fixed bot text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PageError {
    /// The link is refused before any request.
    #[error(
        "the link is not an http or https link, or holds a user part or a malformed GitHub path"
    )]
    Link,
    /// GitHub answers 404: no such page, or a repo the bot cannot read.
    #[error("GitHub found no such public page")]
    GithubMissing,
    /// GitHub answers with another status.
    #[error("GitHub answered with status {0}")]
    GithubStatus(u16),
    /// No answer from GitHub.
    #[error("GitHub did not answer")]
    GithubNoAnswer,
    /// GitHub kept answering with a rate limit.
    #[error("GitHub kept answering with a rate limit")]
    GithubRateLimited,
    /// The GitHub answer is not the expected shape.
    #[error("the GitHub answer is not usable")]
    GithubAnswer,
    /// The fetch service answers with a status other than 200.
    #[error("the fetch service answered with status {0}")]
    FetchStatus(u16),
    /// No answer from the fetch service.
    #[error("the fetch service did not answer")]
    FetchNoAnswer,
    /// The fetch service kept answering with a rate limit.
    #[error("the fetch service kept answering with a rate limit")]
    FetchRateLimited,
    /// The fetch service answer is not the expected JSON.
    #[error("the fetch service answer is not the expected JSON")]
    FetchAnswer,
    /// The page has no text.
    #[error("the page holds no text")]
    Empty,
    /// No `web_fetch` key is set, so only GitHub pages are read.
    #[error("no web_fetch key is set, so the bot reads only GitHub pages")]
    NoFetchKey,
}

/// A `web_fetch` endpoint or key that cannot be used. No variant holds the key.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WebFetchError {
    /// The endpoint is not plain https.
    #[error(
        "the web_fetch URL `{url}` is not https or holds a user part or fragment. Set it to {WEB_FETCH_URL}"
    )]
    Endpoint {
        /// The rejected URL.
        url: String,
    },
    /// The key is empty.
    #[error("the web_fetch API key is empty. Set the Ollama API key, then rerun")]
    EmptyKey,
}

/// The `web_fetch` answer fields the bot reads.
#[derive(Deserialize)]
struct Fetched {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    content: Option<String>,
}

/// Checks that `endpoint` is plain https: no user part and no fragment.
///
/// A config loader calls it before any key is read.
///
/// # Errors
///
/// Returns [`WebFetchError::Endpoint`] when the check fails.
pub fn check_endpoint(endpoint: &Url) -> Result<(), WebFetchError> {
    if endpoint.scheme() != "https"
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.host_str().is_none()
        || endpoint.fragment().is_some()
    {
        return Err(WebFetchError::Endpoint {
            url: endpoint.to_string(),
        });
    }
    Ok(())
}

/// Reads other pages through Ollama `web_fetch`. The service fetches the page.
pub struct WebFetch<'a> {
    transport: &'a dyn Transport,
    paced: Paced<'a>,
    endpoint: String,
    api_key: Secret,
}

impl<'a> WebFetch<'a> {
    /// A reader that posts to `endpoint` with `api_key`. The `pacer` spaces every request.
    ///
    /// # Errors
    ///
    /// Returns a [`WebFetchError`] when `endpoint` is not plain https or `api_key` is empty.
    pub fn new(
        transport: &'a dyn Transport,
        pacer: &'a mut dyn Pacer,
        endpoint: &Url,
        api_key: Secret,
    ) -> Result<Self, WebFetchError> {
        check_endpoint(endpoint)?;
        if api_key.expose().trim().is_empty() {
            return Err(WebFetchError::EmptyKey);
        }
        Ok(Self {
            transport,
            paced: Paced::new(pacer),
            endpoint: endpoint.to_string(),
            api_key,
        })
    }

    /// The title and text of `link`, as the service reads them.
    ///
    /// # Errors
    ///
    /// Returns a [`PageError`] when the service fails, answers with a status
    /// other than 200, or sends an answer that is not the expected JSON.
    pub fn page(&mut self, link: &Url) -> Result<String, PageError> {
        let mut body = Map::new();
        body.insert("url".to_owned(), Value::from(link.as_str()));
        let body = Value::Object(body).to_string().into_bytes();
        let headers = json_bearer(&self.api_key);
        let endpoint = self.endpoint.as_str();
        let transport = self.transport;
        let answer = self
            .paced
            .send(
                endpoint,
                || transport.post(endpoint, &headers, &body),
                &[200],
            )
            .map_err(|error| match error {
                PacedError::Http(_) | PacedError::Body { .. } => PageError::FetchNoAnswer,
                PacedError::Status { status, .. } => PageError::FetchStatus(status),
                PacedError::RetriesExhausted { .. } => PageError::FetchRateLimited,
            })?;
        let fetched: Fetched =
            serde_json::from_slice(&answer.body).map_err(|_| PageError::FetchAnswer)?;
        let content = fetched.content.unwrap_or_default();
        Ok(
            match fetched.title.filter(|title| !title.trim().is_empty()) {
                Some(title) => format!("{title}\n\n{content}"),
                None => content,
            },
        )
    }
}

/// The text of a GitHub page, read through the API with the bot's installation token.
///
/// A README or a file comes as raw bytes. An issue or PR comes as its title,
/// a blank line and its body.
///
/// # Errors
///
/// Returns a [`PageError`] when the page is not found, or the call fails.
pub fn github_page(
    client: &mut RepoClient<'_>,
    target: &GithubTarget,
) -> Result<String, PageError> {
    let text = match target {
        GithubTarget::Readme { repo } => client
            .readme_of(&repo_ref(repo)?)
            .map(|bytes| bytes.map(lossy)),
        GithubTarget::Issue { repo, number } => client
            .issue_of(&repo_ref(repo)?, *number)
            .map(|issue| issue.map(|issue| format!("{}\n\n{}", issue.title, issue.body))),
        GithubTarget::File {
            repo,
            git_ref,
            path,
        } => client
            .file_of(&repo_ref(repo)?, git_ref, path)
            .map(|bytes| bytes.map(lossy)),
    };
    match text {
        Ok(Some(text)) => Ok(text),
        Ok(None) => Err(PageError::GithubMissing),
        Err(error) => Err(api_page_error(&error)),
    }
}

fn repo_ref(repo: &GithubRepo) -> Result<RepoRef, PageError> {
    RepoRef::parse(&format!("{}/{}", repo.owner, repo.name)).map_err(|_| PageError::Link)
}

fn lossy(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

fn api_page_error(error: &ApiError) -> PageError {
    match error {
        ApiError::Status { status, .. } => PageError::GithubStatus(*status),
        ApiError::Http(_) | ApiError::Body { .. } => PageError::GithubNoAnswer,
        ApiError::RetriesExhausted { .. } => PageError::GithubRateLimited,
        _ => PageError::GithubAnswer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_errors_are_fixed_text() {
        assert_eq!(
            PageError::FetchStatus(429).to_string(),
            "the fetch service answered with status 429"
        );
        assert_eq!(
            PageError::GithubMissing.to_string(),
            "GitHub found no such public page"
        );
    }

    #[test]
    fn invalid_bytes_are_replaced() {
        assert_eq!(lossy(b"a\xffb".to_vec()), "a\u{fffd}b");
        assert_eq!(lossy(b"ok".to_vec()), "ok");
    }
}
