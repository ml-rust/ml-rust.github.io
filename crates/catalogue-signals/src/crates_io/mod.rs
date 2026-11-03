//! crates.io API lookups for single crates.
//!
//! [`CratesIo`] reads the owners of one crate from the crates.io API. One
//! client spaces its requests with one [`Pacer`](crate::http::Pacer), so pass
//! an [`IntervalPacer`](crate::http::IntervalPacer) set to
//! [`CRATES_IO_INTERVAL`]. Bulk data comes from the database dump, never from
//! this API.

mod error;
mod owners;
mod pacer;

pub use error::CratesIoError;
pub use owners::{CrateOwner, CratesIo, OWNERS_ENDPOINT};
pub use pacer::CRATES_IO_INTERVAL;
