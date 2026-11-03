//! Access to the POST bodies a fake transport records.

use crate::transport::FakeTransport;

impl FakeTransport {
    /// The body of each POST request, in order.
    pub fn bodies(&self) -> Vec<Vec<u8>> {
        self.posted.borrow().clone()
    }
}
