//! The errors of the history store.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// A history directory that cannot be read or written.
#[derive(Debug, Error)]
pub enum HistoryError {
    /// The history directory cannot be listed.
    #[error("cannot list history directory {}: {source}. Check the path and its permissions, then rerun", path.display())]
    List {
        /// The history directory.
        path: PathBuf,
        /// The cause.
        source: io::Error,
    },
    /// A snapshot file cannot be read.
    #[error("cannot read history file {}: {source}. Check the file permissions, then rerun", path.display())]
    Read {
        /// The snapshot file.
        path: PathBuf,
        /// The cause.
        source: io::Error,
    },
    /// A snapshot file is not valid gzip JSON lines.
    #[error("history file {} is corrupt{}: {reason}. Restore the file from git or remove it, then rerun", path.display(), line_note(*line))]
    Corrupt {
        /// The snapshot file.
        path: PathBuf,
        /// The 1-based line of the fault. `None` when the gzip stream is at fault.
        line: Option<usize>,
        /// What is wrong.
        reason: String,
    },
    /// A snapshot cannot be written.
    #[error("cannot write history file {}: {source}. Check that the directory is writable, then rerun", path.display())]
    Write {
        /// The file or directory being written.
        path: PathBuf,
        /// The cause.
        source: io::Error,
    },
}

/// The ` at line N` part of a corrupt-file message.
fn line_note(line: Option<usize>) -> String {
    line.map(|number| format!(" at line {number}"))
        .unwrap_or_default()
}
