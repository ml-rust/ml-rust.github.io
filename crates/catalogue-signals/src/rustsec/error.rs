//! Errors from reading the RustSec advisory archive.

use thiserror::Error;

/// An archive the reader rejects.
#[derive(Debug, Error)]
pub enum RustsecError {
    /// The archive stream or one of its entries cannot be read.
    #[error(
        "cannot read `{entry}` in the advisory archive: {source}. Run `catalogue rustsec fetch` again and retry"
    )]
    Io {
        /// The entry path, or `archive` for the stream itself.
        entry: String,
        /// The read error.
        source: std::io::Error,
    },
    /// The archive holds no advisory file.
    #[error(
        "the advisory archive holds no `crates/<name>/*.md` file. Check that the input is the rustsec/advisory-db archive"
    )]
    Empty,
}
