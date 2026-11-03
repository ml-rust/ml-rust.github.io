//! The error of a `catalogue dump` action and its exit codes.

use std::io;
use std::path::PathBuf;

use catalogue_signals::dump::DumpError;
use catalogue_signals::fetch::FetchError;
use thiserror::Error;

use crate::cache_dir::CacheDirError;
use crate::exit::{INVALID, IO};

/// A `catalogue dump` action that fails.
#[derive(Debug, Error)]
pub enum DumpCommandError {
    /// The cache directory cannot be derived.
    #[error(transparent)]
    CacheDir(#[from] CacheDirError),
    /// The download fails.
    #[error(transparent)]
    Fetch(#[from] FetchError),
    /// The cached dump file is absent.
    #[error("no dump at {}. Run `catalogue dump fetch` first", .path.display())]
    Missing {
        /// The expected dump file.
        path: PathBuf,
    },
    /// The cached dump file cannot be opened.
    #[error("{}: {source}. Check the path and its permissions, then rerun", .path.display())]
    Open {
        /// The dump file.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// The dump content is rejected.
    #[error("{}: {source}", .path.display())]
    Read {
        /// The dump file.
        path: PathBuf,
        /// The reader error.
        source: DumpError,
    },
    /// A named crate is absent from the dump.
    #[error(
        "no crate named {} in the dump. Check each name against crates.io, then rerun",
        .names.join(", ")
    )]
    UnknownCrates {
        /// The names with no crate.
        names: Vec<String>,
    },
}

impl DumpCommandError {
    /// The process exit code: 1 for dump content, 2 for IO and network.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Read { source, .. } if !matches!(source, DumpError::Io { .. }) => INVALID,
            Self::UnknownCrates { .. } => INVALID,
            _ => IO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_content_errors_exit_with_1_and_read_errors_with_2() {
        let bad = DumpCommandError::Read {
            path: PathBuf::from("d"),
            source: DumpError::MissingTable { table: "crates" },
        };
        assert_eq!(bad.exit_code(), INVALID);
        let io = DumpCommandError::Read {
            path: PathBuf::from("d"),
            source: DumpError::Io {
                entry: "archive".to_owned(),
                source: io::Error::other("x"),
            },
        };
        assert_eq!(io.exit_code(), IO);
    }

    #[test]
    fn unknown_crates_are_listed_and_exit_with_1() {
        let error = DumpCommandError::UnknownCrates {
            names: vec!["nope".to_owned(), "gone".to_owned()],
        };
        let message = error.to_string();
        assert!(message.contains("nope, gone"), "{message}");
        assert_eq!(error.exit_code(), INVALID);
    }
}
