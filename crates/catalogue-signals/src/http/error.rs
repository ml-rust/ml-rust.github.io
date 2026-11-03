//! Errors from the HTTP seam.

use thiserror::Error;

/// A request that sends no response.
#[derive(Debug, Error)]
pub enum HttpError {
    /// The HTTP client cannot be built.
    #[error(
        "cannot build the HTTP client: {reason}. Check the TLS and proxy environment and retry"
    )]
    Client {
        /// What the client builder reports.
        reason: String,
    },
    /// A header value holds bytes HTTP does not allow, such as a newline.
    ///
    /// The message names the header only. The value can be a secret.
    #[error(
        "the value of the `{name}` header holds a character HTTP does not allow. Check the value and rerun"
    )]
    InvalidHeader {
        /// The header name.
        name: String,
    },
    /// A request fails before the server answers.
    #[error("request to {url} failed: {source}. Check the network and rerun")]
    Request {
        /// The requested URL.
        url: String,
        /// The transport error.
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
