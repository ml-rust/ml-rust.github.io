//! The HTTP seam shared by every fetcher.
//!
//! [`Transport`] sends one GET or POST request. [`ReqwestTransport`] is the real
//! implementation and sends the crate `User-Agent`. It adds only the headers
//! the caller passes and reads no environment variable. [`Pacer`] holds
//! requests back to an API's rate limit.

mod client;
mod encode;
mod error;
mod headers;
mod pace;
mod paced;
mod transport;

pub use client::ReqwestTransport;
pub(crate) use encode::percent_encode;
pub use error::HttpError;
pub use headers::Headers;
pub use pace::{IntervalPacer, Pacer};
pub(crate) use paced::Paced;
pub use paced::PacedError;
pub use transport::{Response, Transport};
