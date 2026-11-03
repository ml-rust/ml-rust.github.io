//! Access to the URLs a fake transport records.

use crate::transport::FakeTransport;

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
