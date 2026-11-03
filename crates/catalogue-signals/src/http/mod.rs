//! The HTTP seam shared by every fetcher.
//!
//! [`Transport`] sends one GET or POST request. [`ReqwestTransport`] is the real
//! implementation and sends the crate `User-Agent`. It adds only the headers
//! the caller passes and reads no environment variable.

mod client;
mod error;
mod headers;
mod transport;

pub use client::ReqwestTransport;
pub use error::HttpError;
pub use headers::Headers;
pub use transport::{Response, Transport};
