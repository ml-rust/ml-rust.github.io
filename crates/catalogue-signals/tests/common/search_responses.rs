//! Canned responses and URL access for the GitHub search.

use crate::transport::{FakeResponse, FakeTransport};

impl FakeResponse {
    /// A response with `status`, the given headers and a text body.
    pub fn with_body(status: u16, headers: &[(&str, &str)], body: &str) -> Self {
        Self {
            status,
            headers: headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            body: body.as_bytes().to_vec(),
        }
    }
}

impl FakeTransport {
    /// The URL of each request, in order.
    pub fn urls(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|(url, _)| url.clone())
            .collect()
    }
}
