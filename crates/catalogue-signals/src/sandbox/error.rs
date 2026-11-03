//! The errors of the sandbox check files.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// A sandbox list or result file that cannot be read.
#[derive(Debug, Error)]
pub enum SandboxError {
    /// The file cannot be read.
    #[error("cannot read sandbox file {}: {source}. Check the path and its permissions, then rerun", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// The cause.
        source: io::Error,
    },
    /// A line is not a valid record.
    #[error("sandbox file {} is corrupt at line {line}: {reason}. Write the file again with `catalogue sandbox`, then rerun", path.display())]
    Corrupt {
        /// The file.
        path: PathBuf,
        /// The 1-based line of the fault.
        line: usize,
        /// What is wrong.
        reason: String,
    },
}
