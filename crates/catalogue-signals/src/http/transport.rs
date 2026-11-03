//! The [`Transport`] trait and the [`Response`] it returns.

use std::io::Read;

use super::error::HttpError;
use super::headers::Headers;

/// One HTTP response with a streaming body.
pub struct Response {
    /// The HTTP status code.
    pub status: u16,
    /// The response headers.
    pub headers: Headers,
    /// The body, read as it arrives.
    pub body: Box<dyn Read>,
}

/// Sends GET and POST requests. Redirects are followed inside the implementation.
pub trait Transport {
    /// Fetches `url` with the extra request `headers` as name and value pairs.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Request`] when no response arrives.
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, HttpError>;

    /// Posts `body` to `url` with the extra request `headers` as name and value pairs.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Request`] when no response arrives.
    fn post(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError>;
}
