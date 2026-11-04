//! Counts the contributors of one repo with a single REST request.

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use url::Url;

use super::error::GithubError;
use super::facts::RepoRef;
use super::headers::rest_headers;
use super::json::parse_json;
use crate::http::{Paced, Pacer, Transport, last_link, percent_encode};

/// The repos endpoint of the GitHub API.
const ENDPOINT: &str = "https://api.github.com/repos";

/// The answer GitHub gives a repo whose contributor list it will not compute.
const TOO_LARGE: &str = "too large";

/// The status of a repo with no commits.
const NO_CONTENT: u16 = 204;

/// The status GitHub gives for a repo whose list is too large, and for other refusals.
const FORBIDDEN: u16 = 403;

/// What GitHub says about the contributors of one repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Contributors {
    /// The count of contributors with a GitHub account.
    Counted {
        /// The count.
        count: u64,
    },
    /// GitHub refuses to list them: the history is too large.
    TooLarge,
}

/// Counts the contributors of `repo` with one request.
///
/// The request asks for one contributor per page, so the page number of the `last` link is
/// the count. An answer with no `last` link holds every contributor in its body. `token` is
/// sent as a bearer token. A 403 or 429 answer with a wait hint is retried after the `pacer`
/// waits.
///
/// # Errors
///
/// Returns a [`GithubError`] when no response arrives, the status is not 200, 204 or a 403
/// that says the list is too large, the body breaks off or is not a JSON list, the `Link`
/// header names no page, or the rate limit stays after the retries.
pub fn repo_contributors(
    transport: &dyn Transport,
    token: &str,
    repo: &RepoRef,
    pacer: &mut dyn Pacer,
) -> Result<Contributors, GithubError> {
    let url = format!(
        "{ENDPOINT}/{}/{}/contributors?per_page=1&anon=false",
        percent_encode(&repo.owner),
        percent_encode(&repo.name)
    );
    let headers = rest_headers(token, "application/vnd.github+json");
    let mut paced = Paced::new(pacer);
    let answer = paced.send_or_refused(
        &url,
        || transport.get(&url, &headers),
        &[200, NO_CONTENT],
        &[FORBIDDEN],
    )?;
    if answer.status == FORBIDDEN {
        let body = String::from_utf8_lossy(&answer.body).to_ascii_lowercase();
        return if body.contains(TOO_LARGE) {
            Ok(Contributors::TooLarge)
        } else {
            Err(GithubError::status(&url, FORBIDDEN))
        };
    }
    if answer.status == NO_CONTENT {
        return Ok(Contributors::Counted { count: 0 });
    }
    if let Some(link) = answer.headers.get("link")
        && let Some(last) = last_link(link)
    {
        let count = page_number(last).ok_or_else(|| GithubError::BadLink {
            url: url.clone(),
            value: link.to_owned(),
        })?;
        return Ok(Contributors::Counted { count });
    }
    let listed: Vec<IgnoredAny> = parse_json(&url, &answer.body)?;
    let count = u64::try_from(listed.len()).unwrap_or(u64::MAX);
    Ok(Contributors::Counted { count })
}

/// The `page` query parameter of the link `url`.
fn page_number(url: &str) -> Option<u64> {
    let parsed = Url::parse(url).ok()?;
    let (_, page) = parsed.query_pairs().find(|(name, _)| name == "page")?;
    page.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_comes_from_the_page_parameter_alone() {
        let url =
            "https://api.github.com/repositories/1/contributors?per_page=1&anon=false&page=38";
        assert_eq!(page_number(url), Some(38));
        assert_eq!(
            page_number("https://x/y?per_page=1"),
            None,
            "per_page is not page"
        );
        assert_eq!(page_number("https://x/y?page=many"), None);
        assert_eq!(page_number("not a url"), None);
    }
}
