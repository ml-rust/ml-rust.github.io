//! The HTTP seam shared by every fetcher.
//!
//! [`Transport`] sends one GET, POST, PATCH or DELETE request.
//! [`ReqwestTransport`] is the real implementation and sends the crate
//! `User-Agent`. It adds only the headers the caller passes and reads no
//! environment variable. [`Pacer`] holds requests back to an API's rate limit.
//! [`Paced`] sends through a pacer and retries rate-limit answers.
//! [`next_link`] and [`last_link`] read a page from a `Link` header.

mod client;
mod encode;
mod error;
mod headers;
mod link;
mod pace;
mod paced;
mod transport;

pub use client::ReqwestTransport;
pub use encode::percent_encode;
pub use error::HttpError;
pub use headers::Headers;
pub use link::{last_link, next_link};
pub use pace::{IntervalPacer, Pacer};
pub use paced::{Answer, Paced, PacedError};
pub use transport::{Response, Transport};
