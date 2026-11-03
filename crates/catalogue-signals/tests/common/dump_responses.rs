//! Canned responses for the database dump download.

use crate::transport::FakeResponse;

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

    /// Replaces `Content-Length` with `length`.
    pub fn announcing(mut self, length: u64) -> Self {
        self.headers
            .retain(|(name, _)| !name.eq_ignore_ascii_case("content-length"));
        self.headers
            .push(("Content-Length".to_owned(), length.to_string()));
        self
    }
}
