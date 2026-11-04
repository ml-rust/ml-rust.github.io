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
pub const MAX_BODY: u64 = 32 << 20;

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

/// A response whose status the caller accepts, with its body read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The HTTP status code.
    pub status: u16,
    /// The response headers.
    pub headers: Headers,
    /// The body, up to the size cap.
    pub body: Vec<u8>,
}

/// Sends requests one at a time and holds each back until the [`Pacer`]
/// allows it.
pub struct Paced<'a> {
    pacer: &'a mut dyn Pacer,
    /// The headers of the last response, until the pacer has seen them.
    previous: Option<Headers>,
    requests: u32,
}

impl<'a> Paced<'a> {
    /// A sender that has made no request.
    pub fn new(pacer: &'a mut dyn Pacer) -> Self {
        Self {
            pacer,
            previous: None,
            requests: 0,
        }
    }

    /// The requests sent, retries included.
    pub fn requests(&self) -> u32 {
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
    pub fn fetch(
        &mut self,
        url: &str,
        send: impl FnMut() -> Result<Response, HttpError>,
    ) -> Result<Vec<u8>, PacedError> {
        match self.run(url, send, &[])? {
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
    pub fn fetch_optional(
        &mut self,
        url: &str,
        send: impl FnMut() -> Result<Response, HttpError>,
    ) -> Result<Option<Vec<u8>>, PacedError> {
        self.run(url, send, &[404])
    }

    /// Like [`fetch`](Self::fetch), but an answer with a status in `absent` gives `None`.
    ///
    /// # Errors
    ///
    /// Returns a [`PacedError`] as [`fetch`](Self::fetch) does.
    pub fn fetch_absent(
        &mut self,
        url: &str,
        send: impl FnMut() -> Result<Response, HttpError>,
        absent: &[u16],
    ) -> Result<Option<Vec<u8>>, PacedError> {
        self.run(url, send, absent)
    }

    /// Sends the request that `send` builds and returns an answer whose status is in `accepted`.
    ///
    /// The answer keeps its status and headers, so a caller can read a
    /// `Link` header or tell 201 from 200. Retries work as in
    /// [`fetch`](Self::fetch).
    ///
    /// # Errors
    ///
    /// Returns a [`PacedError`] when no response arrives, the status is not in
    /// `accepted`, the body breaks off, or the rate limit stays after the retries.
    pub fn send(
        &mut self,
        url: &str,
        send: impl FnMut() -> Result<Response, HttpError>,
        accepted: &[u16],
    ) -> Result<Answer, PacedError> {
        match self.exchange(url, send, accepted, &[], &[], MAX_BODY)? {
            Some(answer) => Ok(answer),
            None => Err(PacedError::Status {
                url: url.to_owned(),
                status: 404,
            }),
        }
    }

    /// Like [`send`](Self::send), but also gives an answer with a status in `refused`, so a
    /// caller can read why the server refused.
    ///
    /// A status in `refused` that carries a wait hint stays a rate limit and is retried.
    ///
    /// # Errors
    ///
    /// Returns a [`PacedError`] as [`send`](Self::send) does.
    pub fn send_or_refused(
        &mut self,
        url: &str,
        send: impl FnMut() -> Result<Response, HttpError>,
        accepted: &[u16],
        refused: &[u16],
    ) -> Result<Answer, PacedError> {
        match self.exchange(url, send, accepted, &[], refused, MAX_BODY)? {
            Some(answer) => Ok(answer),
            None => Err(PacedError::Status {
                url: url.to_owned(),
                status: 404,
            }),
        }
    }

    /// Sends the request that `send` builds and reads at most `limit` bytes of a 200 answer.
    ///
    /// An answer with a status in `absent` gives `None`. The answer keeps its headers. A body
    /// past `limit` stops at `limit` with no error. Retries work as in [`fetch`](Self::fetch).
    ///
    /// # Errors
    ///
    /// Returns a [`PacedError`] as [`fetch`](Self::fetch) does.
    pub fn fetch_capped(
        &mut self,
        url: &str,
        send: impl FnMut() -> Result<Response, HttpError>,
        absent: &[u16],
        limit: u64,
    ) -> Result<Option<Answer>, PacedError> {
        self.exchange(url, send, &[200], absent, &[], limit)
    }

    fn run(
        &mut self,
        url: &str,
        send: impl FnMut() -> Result<Response, HttpError>,
        absent: &[u16],
    ) -> Result<Option<Vec<u8>>, PacedError> {
        Ok(self
            .exchange(url, send, &[200], absent, &[], MAX_BODY)?
            .map(|answer| answer.body))
    }

    /// Sends until an answer arrives. A status in `accepted` gives the answer, a status in
    /// `absent` gives `None`, a status in `refused` without a wait hint gives the answer. The
    /// answer holds at most `limit` bytes of the body.
    fn exchange(
        &mut self,
        url: &str,
        mut send: impl FnMut() -> Result<Response, HttpError>,
        accepted: &[u16],
        absent: &[u16],
        refused: &[u16],
        limit: u64,
    ) -> Result<Option<Answer>, PacedError> {
        let mut retries = 0;
        loop {
            if let Some(headers) = self.previous.take() {
                self.pacer.wait(&headers);
            }
            self.requests += 1;
            let response = send()?;
            let headers = response.headers;
            let limited = is_rate_limited(response.status, &headers);
            if accepted.contains(&response.status)
                || (refused.contains(&response.status) && !limited)
            {
                self.previous = Some(headers.clone());
                let body = read_body(url, response.body, limit)?;
                return Ok(Some(Answer {
                    status: response.status,
                    headers,
                    body,
                }));
            }
            if absent.contains(&response.status) {
                self.previous = Some(headers);
                return Ok(None);
            }
            if !limited {
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

/// Reads a response body up to `limit` bytes.
fn read_body(url: &str, body: Box<dyn Read>, limit: u64) -> Result<Vec<u8>, PacedError> {
    let mut bytes = Vec::new();
    body.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|source| PacedError::Body {
            url: url.to_owned(),
            source,
        })?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    /// Counts the waits.
    #[derive(Default)]
    struct Counting(u32);

    impl Pacer for Counting {
        fn wait(&mut self, _response_headers: &Headers) {
            self.0 += 1;
        }
    }

    fn response(status: u16) -> Result<Response, HttpError> {
        Ok(Response {
            status,
            headers: Headers::new([("Link", "<https://x/2>; rel=\"next\"")]),
            body: Box::new(Cursor::new(b"{}".to_vec())),
        })
    }

    #[test]
    fn send_returns_an_accepted_answer_with_its_status_and_headers() {
        let mut pacer = Counting::default();
        let mut paced = Paced::new(&mut pacer);
        let answer = paced
            .send("u", || response(201), &[200, 201])
            .expect("accepted");
        assert_eq!(answer.status, 201);
        assert_eq!(
            answer.headers.get("link"),
            Some("<https://x/2>; rel=\"next\"")
        );
        assert_eq!(answer.body, b"{}");
        paced.send("u", || response(204), &[204]).expect("accepted");
        assert_eq!(paced.requests(), 2);
        drop(paced);
        assert_eq!(pacer.0, 1);
    }

    #[test]
    fn send_rejects_a_status_outside_the_accepted_list() {
        let mut pacer = Counting::default();
        let mut paced = Paced::new(&mut pacer);
        let error = paced
            .send("u", || response(200), &[201])
            .expect_err("rejected");
        assert!(matches!(error, PacedError::Status { status: 200, .. }));
    }

    #[test]
    fn a_refusal_gives_its_answer_and_a_rate_limit_is_retried() {
        let mut pacer = Counting::default();
        let mut paced = Paced::new(&mut pacer);
        let answer = paced
            .send_or_refused("u", || response(403), &[200], &[403])
            .expect("refused");
        assert_eq!(answer.status, 403);
        let limited = || {
            Ok(Response {
                status: 403,
                headers: Headers::new([("Retry-After", "1")]),
                body: Box::new(Cursor::new(Vec::new())),
            })
        };
        let error = paced
            .send_or_refused("u", limited, &[200], &[403])
            .expect_err("still limited");
        assert!(matches!(
            error,
            PacedError::RetriesExhausted { retries: 3, .. }
        ));
    }

    #[test]
    fn fetch_capped_stops_at_the_limit_and_keeps_the_headers() {
        let mut pacer = Counting::default();
        let mut paced = Paced::new(&mut pacer);
        let answer = paced
            .fetch_capped("u", || response(200), &[404], 1)
            .expect("answers")
            .expect("present");
        assert_eq!(answer.body, b"{");
        assert!(answer.headers.get("link").is_some());
        let absent = paced
            .fetch_capped("u", || response(404), &[404], 1)
            .expect("answers");
        assert_eq!(absent, None);
    }

    #[test]
    fn fetch_accepts_only_200() {
        let mut pacer = Counting::default();
        let mut paced = Paced::new(&mut pacer);
        assert!(paced.fetch("u", || response(200)).is_ok());
        let error = paced.fetch("u", || response(201)).expect_err("rejected");
        assert!(matches!(error, PacedError::Status { status: 201, .. }));
    }

    #[test]
    fn rate_limit_answers_need_a_wait_hint() {
        let hint = Headers::new([("Retry-After", "3")]);
        let empty = Headers::default();
        let spent = Headers::new([("x-ratelimit-remaining", "0")]);
        assert!(is_rate_limited(403, &hint) && is_rate_limited(429, &spent));
        assert!(!is_rate_limited(403, &empty) && !is_rate_limited(500, &hint));
    }
}
