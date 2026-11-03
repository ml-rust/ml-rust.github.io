//! The error of a `catalogue rustsec` action and its exit codes.

use std::io;
use std::path::PathBuf;

use catalogue_signals::fetch::FetchError;
use catalogue_signals::rustsec::RustsecError;
use thiserror::Error;

use crate::cache_dir::CacheDirError;
use crate::exit::{INVALID, IO};

/// A `catalogue rustsec` action that fails.
#[derive(Debug, Error)]
pub enum RustsecCommandError {
    /// The cache directory cannot be derived.
    #[error(transparent)]
    CacheDir(#[from] CacheDirError),
    /// The download fails.
    #[error(transparent)]
    Fetch(#[from] FetchError),
    /// The cached archive is absent.
    #[error("no advisory archive at {}. Run `catalogue rustsec fetch` first", .path.display())]
    Missing {
        /// The expected archive file.
        path: PathBuf,
    },
    /// The cached archive cannot be opened.
    #[error("{}: {source}. Check the path and its permissions, then rerun", .path.display())]
    Open {
        /// The archive file.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// The archive content is rejected.
    #[error("{}: {source}", .path.display())]
    Read {
        /// The archive file.
        path: PathBuf,
        /// The reader error.
        source: RustsecError,
    },
}

impl RustsecCommandError {
    /// The process exit code: 1 for archive content, 2 for IO and network.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Read { source, .. } if !matches!(source, RustsecError::Io { .. }) => INVALID,
            _ => IO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_content_errors_exit_with_1_and_read_errors_with_2() {
        let empty = RustsecCommandError::Read {
            path: PathBuf::from("a"),
            source: RustsecError::Empty,
        };
        assert_eq!(empty.exit_code(), INVALID);
        let io = RustsecCommandError::Read {
            path: PathBuf::from("a"),
            source: RustsecError::Io {
                entry: "archive".to_owned(),
                source: io::Error::other("x"),
            },
        };
        assert_eq!(io.exit_code(), IO);
    }
}
