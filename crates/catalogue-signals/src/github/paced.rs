//! Paced requests with rate-limit retries, shared by the search and the facts.

use std::io::Read;

use serde::de::DeserializeOwned;

use super::error::GithubError;
use super::pacer::Pacer;
use crate::http::{Headers, HttpError, Response};

/// The retries after a rate-limit answer.
const MAX_RETRIES: u32 = 3;
/// The longest response body read, in bytes.
const MAX_BODY: u64 = 32 << 20;

/// Sends requests one at a time and holds each back until the [`Pacer`]
/// allows it.
pub(super) struct Paced<'a> {
    pacer: &'a mut dyn Pacer,
    /// The headers of the last response, until the pacer has seen them.
    previous: Option<Headers>,
    requests: u32,
}

impl<'a> Paced<'a> {
    /// A sender that has made no request.
    pub(super) fn new(pacer: &'a mut dyn Pacer) -> Self {
        Self {
            pacer,
            previous: None,
            requests: 0,
        }
    }

    /// The requests sent, retries included.
    pub(super) fn requests(&self) -> u32 {
        self.requests
    }

    /// Sends the request that `send` builds and returns the body of a 200 answer.
    ///
    /// A 403 or 429 answer with a wait hint is retried after the pacer waits.
    /// `url` names the request in errors.
    ///
    /// # Errors
    ///
    /// Returns a [`GithubError`] when no response arrives, the status is not
    /// 200, the body breaks off, or the rate limit stays after the retries.
    pub(super) fn fetch(
        &mut self,
        url: &str,
        mut send: impl FnMut() -> Result<Response, HttpError>,
    ) -> Result<Vec<u8>, GithubError> {
        let mut retries = 0;
        loop {
            if let Some(headers) = self.previous.take() {
                self.pacer.wait(&headers);
            }
            self.requests += 1;
            let response = send()?;
            let headers = response.headers;
            if response.status == 200 {
                self.previous = Some(headers);
                return read_body(url, response.body);
            }
            if !is_rate_limited(response.status, &headers) {
                return Err(GithubError::status(url, response.status));
            }
            if retries == MAX_RETRIES {
                return Err(GithubError::RetriesExhausted {
                    url: url.to_owned(),
                    retries,
                });
            }
            retries += 1;
            self.previous = Some(headers);
        }
    }
}

/// Parses a response body as JSON.
pub(super) fn parse_json<T: DeserializeOwned>(url: &str, bytes: &[u8]) -> Result<T, GithubError> {
    serde_json::from_slice(bytes).map_err(|source| GithubError::Json {
        url: url.to_owned(),
        source,
    })
}

/// Whether a 403 or 429 answer asks the caller to wait and retry.
fn is_rate_limited(status: u16, headers: &Headers) -> bool {
    matches!(status, 403 | 429)
        && (headers.get("retry-after").is_some()
            || headers
                .get("x-ratelimit-remaining")
                .is_some_and(|value| value.trim() == "0"))
}

/// Reads a response body up to the size cap.
fn read_body(url: &str, body: Box<dyn Read>) -> Result<Vec<u8>, GithubError> {
    let mut bytes = Vec::new();
    body.take(MAX_BODY)
        .read_to_end(&mut bytes)
        .map_err(|source| GithubError::Body {
            url: url.to_owned(),
            source,
        })?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_answers_need_a_wait_hint() {
        let hint = Headers::new([("Retry-After", "3")]);
        let empty = Headers::default();
        let spent = Headers::new([("x-ratelimit-remaining", "0")]);
        assert!(is_rate_limited(403, &hint) && is_rate_limited(429, &spent));
        assert!(!is_rate_limited(403, &empty) && !is_rate_limited(500, &hint));
    }
}
