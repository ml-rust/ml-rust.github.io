//! Resumable download of the crates.io database dump.
//!
//! [`dump`] keeps `db-dump.tar.gz` in a cache directory. It revalidates a
//! complete file with the stored `ETag` and resumes an interrupted download.
//! The HTTP layer sits behind [`Transport`], so [`dump_with`] runs without a
//! network.

mod download;
mod error;
mod http;
mod state;
mod transport;

pub use download::{FetchOutcome, dump, dump_dir, dump_path, dump_with};
pub use error::FetchError;
pub use state::DumpState;
pub use transport::{Headers, Response, Transport};
