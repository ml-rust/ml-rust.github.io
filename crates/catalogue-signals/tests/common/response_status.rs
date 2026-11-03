//! A status-only canned response.

use crate::transport::FakeResponse;

impl FakeResponse {
    /// A response with only a status.
    pub fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
}
