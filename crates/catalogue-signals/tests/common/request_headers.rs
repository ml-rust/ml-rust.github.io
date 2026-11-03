//! Access to the extra headers a fake transport records.

use crate::transport::FakeTransport;

impl FakeTransport {
    /// The extra headers of each request, in order.
    pub fn requests(&self) -> Vec<Vec<(String, String)>> {
        self.calls
            .borrow()
            .iter()
            .map(|(_, headers)| headers.clone())
            .collect()
    }
}
