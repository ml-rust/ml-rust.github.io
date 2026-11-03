//! A scripted transport and a temporary cache directory for the fetch tests.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Cursor;
use std::path::PathBuf;

use catalogue_signals::fetch::{FetchError, Headers, Response, Transport};
use tempfile::TempDir;

/// A temporary cache directory under the build directory.
pub fn cache() -> (TempDir, PathBuf) {
    let guard = TempDir::new_in(env!("CARGO_TARGET_TMPDIR")).expect("temp dir");
    let path = guard.path().to_path_buf();
    (guard, path)
}

/// One canned HTTP response.
pub struct FakeResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl FakeResponse {
    /// A `200` with `body`, an `ETag` and a matching `Content-Length`.
    pub fn ok(body: &[u8], etag: &str) -> Self {
        Self {
            status: 200,
            headers: vec![
                ("ETag".to_owned(), etag.to_owned()),
                ("Content-Length".to_owned(), body.len().to_string()),
            ],
            body: body.to_vec(),
        }
    }

    /// A `206` with `body` and the given `Content-Range`.
    pub fn partial(body: &[u8], content_range: &str) -> Self {
        Self {
            status: 206,
            headers: vec![("Content-Range".to_owned(), content_range.to_owned())],
            body: body.to_vec(),
        }
    }

    /// A response with only a status.
    pub fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Replaces `Content-Length` with `length`.
    pub fn announcing(mut self, length: u64) -> Self {
        self.headers
            .retain(|(name, _)| !name.eq_ignore_ascii_case("content-length"));
        self.headers
            .push(("Content-Length".to_owned(), length.to_string()));
        self
    }
}

/// A transport that replays canned responses and records request headers.
pub struct FakeTransport {
    responses: RefCell<VecDeque<FakeResponse>>,
    requests: RefCell<Vec<Vec<(String, String)>>>,
}

impl FakeTransport {
    pub fn new(responses: impl IntoIterator<Item = FakeResponse>) -> Self {
        Self {
            responses: RefCell::new(responses.into_iter().collect()),
            requests: RefCell::new(Vec::new()),
        }
    }

    /// The extra headers of each request, in order.
    pub fn requests(&self) -> Vec<Vec<(String, String)>> {
        self.requests.borrow().clone()
    }
}

impl Transport for FakeTransport {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, FetchError> {
        self.requests.borrow_mut().push(
            headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
        );
        let next = self.responses.borrow_mut().pop_front();
        let fake = next.ok_or_else(|| FetchError::Request {
            url: url.to_owned(),
            source: "no canned response left".into(),
        })?;
        Ok(Response {
            status: fake.status,
            headers: Headers::new(fake.headers),
            body: Box::new(Cursor::new(fake.body)),
        })
    }
}
