//! Resumable downloads into the cache directory.
//!
//! [`cached`] keeps one file and its `state.json` in a directory. It
//! revalidates a complete file with the stored `ETag` and resumes an
//! interrupted download. [`dump`] fetches the crates.io database dump and
//! [`rustsec`] fetches the RustSec advisory archive. The HTTP layer sits behind
//! [`Transport`](crate::http::Transport), so [`cached`] runs without a network.

mod download;
mod error;
mod range;
mod sources;
mod state;

pub use download::{FetchOutcome, cached};
pub use error::FetchError;
pub use sources::{
    dump, dump_dir, dump_path, dump_with, rustsec, rustsec_dir, rustsec_path, rustsec_with,
};
pub use state::DumpState;
