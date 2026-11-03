//! Paced requests with rate-limit retries.

use std::io::Read;

use thiserror::Error;

use super::error::HttpError;
use super::headers::Headers;
use super::pace::Pacer;
use super::transport::Response;

/// The retries after a rate-limit answer.
const MAX_RETRIES: u32 = 3;
/// The longest response body read, in bytes.
const MAX_BODY: u64 = 32 << 20;

/// A paced request that ends without a usable body.
#[derive(Debug, Error)]
pub enum PacedError {
    /// The HTTP layer fails before the server answers.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The server answers with an unexpected status.
    #[error("{url} answered with status {status}")]
    Status {
        /// The requested URL.
        url: String,
        /// The HTTP status code.
        status: u16,
    },
    /// The response body breaks off.
    #[error("reading the body from {url} failed: {source}")]
    Body {
        /// The requested URL.
        url: String,
        /// The read error.
        source: std::io::Error,
    },
    /// The server keeps asking the caller to wait.
    #[error("{url} still answered with a rate limit after {retries} retries")]
    RetriesExhausted {
        /// The requested URL.
        url: String,
        /// The retries made after the first request.
        retries: u32,
    },
}

/// Sends requests one at a time and holds each back until the [`Pacer`]
/// allows it.
pub(crate) struct Paced<'a> {
    pacer: &'a mut dyn Pacer,
    /// The headers of the last response, until the pacer has seen them.
    previous: Option<Headers>,
    requests: u32,
}

impl<'a> Paced<'a> {
    /// A sender that has made no request.
    pub(crate) fn new(pacer: &'a mut dyn Pacer) -> Self {
        Self {
            pacer,
            previous: None,
            requests: 0,
        }
    }

    /// The requests sent, retries included.
    pub(crate) fn requests(&self) -> u32 {
        self.requests
    }

    /// Sends the request that `send` builds and returns the body of a 200 answer.
    ///
    /// A 403 or 429 answer with a wait hint is retried after the pacer waits.
    /// `url` names the request in errors.
    ///
    /// # Errors
    ///
    /// Returns a [`PacedError`] when no response arrives, the status is not
    /// 200, the body breaks off, or the rate limit stays after the retries.
    pub(crate) fn fetch(
        &mut self,
        url: &str,
        send: impl FnMut() -> Result<Response, HttpError>,
    ) -> Result<Vec<u8>, PacedError> {
        match self.run(url, send, false)? {
            Some(bytes) => Ok(bytes),
            None => Err(PacedError::Status {
                url: url.to_owned(),
                status: 404,
            }),
        }
    }

    /// Like [`fetch`](Self::fetch), but a 404 answer gives `None`.
    ///
    /// # Errors
    ///
    /// Returns a [`PacedError`] as [`fetch`](Self::fetch) does.
    pub(crate) fn fetch_optional(
        &mut self,
        url: &str,
        send: impl FnMut() -> Result<Response, HttpError>,
    ) -> Result<Option<Vec<u8>>, PacedError> {
        self.run(url, send, true)
    }

    fn run(
        &mut self,
        url: &str,
        mut send: impl FnMut() -> Result<Response, HttpError>,
        missing_ok: bool,
    ) -> Result<Option<Vec<u8>>, PacedError> {
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
                return read_body(url, response.body).map(Some);
            }
            if missing_ok && response.status == 404 {
                self.previous = Some(headers);
                return Ok(None);
            }
            if !is_rate_limited(response.status, &headers) {
                return Err(PacedError::Status {
                    url: url.to_owned(),
                    status: response.status,
                });
            }
            if retries == MAX_RETRIES {
                return Err(PacedError::RetriesExhausted {
                    url: url.to_owned(),
                    retries,
                });
            }
            retries += 1;
            self.previous = Some(headers);
        }
    }
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
fn read_body(url: &str, body: Box<dyn Read>) -> Result<Vec<u8>, PacedError> {
    let mut bytes = Vec::new();
    body.take(MAX_BODY)
        .read_to_end(&mut bytes)
        .map_err(|source| PacedError::Body {
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
