//! Paced, authenticated REST calls and pagination.

use std::time::Duration;

use catalogue_signals::USER_AGENT;
use catalogue_signals::github::rest_headers;
use catalogue_signals::http::{Answer, Paced, Pacer, Transport, next_link};
use serde::de::DeserializeOwned;

use crate::error::ApiError;
use crate::secret::Secret;

/// The base URL of the GitHub REST API. A next-page link must start with it.
pub const API_URL: &str = "https://api.github.com";

/// The media type of every request.
const ACCEPT: &str = "application/vnd.github+json";

/// The most pages one listing reads. A listing with more is an error.
pub const MAX_PAGES: usize = 50;

/// The shortest gap between two requests. GitHub asks for at least one second
/// between write calls. Pass it to an `IntervalPacer`.
pub const WRITE_INTERVAL: Duration = Duration::from_secs(1);

/// The HTTP method of a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Get,
    Post,
    Patch,
}

impl Verb {
    /// The method name.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Patch => "PATCH",
        }
    }
}

/// Sends REST calls through one pacer.
pub(crate) struct Api<'a> {
    transport: &'a dyn Transport,
    paced: Paced<'a>,
}

impl<'a> Api<'a> {
    /// A sender over `transport` that the `pacer` spaces.
    pub(crate) fn new(transport: &'a dyn Transport, pacer: &'a mut dyn Pacer) -> Self {
        Self {
            transport,
            paced: Paced::new(pacer),
        }
    }

    /// Sends one call with `bearer` and returns an answer whose status is in `accepted`.
    ///
    /// A `body` goes out as JSON.
    pub(crate) fn call(
        &mut self,
        verb: Verb,
        url: &str,
        bearer: &Secret,
        body: Option<&str>,
        accepted: &[u16],
    ) -> Result<Answer, ApiError> {
        let mut headers = rest_headers(bearer.expose(), ACCEPT);
        headers.push(("User-Agent", USER_AGENT.to_owned()));
        if body.is_some() {
            headers.push(("Content-Type", "application/json".to_owned()));
        }
        let bytes = body.unwrap_or_default().as_bytes();
        let transport = self.transport;
        let send = || match verb {
            Verb::Get => transport.get(url, &headers),
            Verb::Post => transport.post(url, &headers, bytes),
            Verb::Patch => transport.patch(url, &headers, bytes),
        };
        self.paced
            .send(url, send, accepted)
            .map_err(|error| ApiError::from_paced(verb.as_str(), error))
    }

    /// Reads every page of the JSON array listing at `url`.
    ///
    /// Pages follow the `rel="next"` link. A link outside [`API_URL`] or a
    /// listing past [`MAX_PAGES`] pages is an error.
    pub(crate) fn get_pages<T: DeserializeOwned>(
        &mut self,
        url: &str,
        bearer: &Secret,
    ) -> Result<Vec<T>, ApiError> {
        let mut items = Vec::new();
        let mut next = Some(url.to_owned());
        let mut pages = 0;
        while let Some(page) = next.take() {
            if pages == MAX_PAGES {
                return Err(ApiError::TooManyPages {
                    url: url.to_owned(),
                    pages: MAX_PAGES,
                });
            }
            pages += 1;
            let answer = self.call(Verb::Get, &page, bearer, None, &[200])?;
            let mut batch: Vec<T> = parse_json(&page, &answer.body)?;
            items.append(&mut batch);
            next = match answer.headers.get("link").and_then(next_link) {
                Some(link) => Some(same_api(&page, link)?.to_owned()),
                None => None,
            };
        }
        Ok(items)
    }
}

/// `link` when it points inside [`API_URL`].
fn same_api<'l>(page: &str, link: &'l str) -> Result<&'l str, ApiError> {
    let inside = link
        .strip_prefix(API_URL)
        .is_some_and(|rest| rest.starts_with('/'));
    if inside {
        Ok(link)
    } else {
        Err(ApiError::ForeignLink {
            url: page.to_owned(),
            link: link.to_owned(),
            api: API_URL,
        })
    }
}

/// Parses a response body from `url` as JSON.
pub(crate) fn parse_json<T: DeserializeOwned>(url: &str, bytes: &[u8]) -> Result<T, ApiError> {
    serde_json::from_slice(bytes).map_err(|source| ApiError::Json {
        url: url.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_links_inside_the_api_are_followed() {
        assert!(same_api("p", "https://api.github.com/repos/o/r/labels?page=2").is_ok());
        for link in [
            "https://api.github.com.evil.example/x",
            "https://evil.example/https://api.github.com/x",
            "http://api.github.com/x",
            "https://api.github.com",
        ] {
            let error = same_api("p", link).expect_err(link);
            assert!(matches!(error, ApiError::ForeignLink { .. }), "{link}");
        }
    }
}
