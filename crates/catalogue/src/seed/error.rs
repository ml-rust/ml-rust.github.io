//! The error of `catalogue seed` and its exit codes.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

use crate::discover::DiscoverError;
use crate::dump::DumpCommandError;
use crate::exit::{INVALID, IO};

/// A `catalogue seed` run that fails.
#[derive(Debug, Error)]
pub enum SeedError {
    /// The data files, the discovery rules or the cache directory break a rule.
    #[error(transparent)]
    Discover(#[from] DiscoverError),
    /// The cached dump cannot be found or read.
    #[error(transparent)]
    Dump(#[from] DumpCommandError),
    /// `--min-users` is unset and the data directory has no `thresholds.toml`.
    #[error("no thresholds at {}. Create the file or pass --min-users <N>, then rerun", .path.display())]
    NoThresholds {
        /// The expected thresholds file.
        path: PathBuf,
    },
    /// An entry file for a draft already exists.
    #[error("{} already exists. Remove or rename it, then rerun", .path.display())]
    Exists {
        /// The entry file that blocks the write.
        path: PathBuf,
    },
    /// A draft cannot be written as TOML.
    #[error("entry `{slug}` cannot be written as TOML: {source}")]
    Serialize {
        /// The slug of the draft.
        slug: String,
        /// The error from the TOML writer.
        source: toml::ser::Error,
    },
    /// The operating system refuses a write.
    #[error("{}: {source}", .path.display())]
    Io {
        /// The file or directory that fails.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
}

impl SeedError {
    /// The process exit code.
    ///
    /// 1 for rule breaks, a missing thresholds file and an existing entry file.
    /// 2 for IO errors. The inner code for discover and dump errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Discover(error) => error.exit_code(),
            Self::Dump(error) => error.exit_code(),
            Self::NoThresholds { .. } | Self::Exists { .. } | Self::Serialize { .. } => INVALID,
            Self::Io { .. } => IO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_carry_their_exit_codes() {
        let exists = SeedError::Exists {
            path: PathBuf::from("data/entries/a.toml"),
        };
        assert_eq!(exists.exit_code(), INVALID);
        assert!(exists.to_string().contains("data/entries/a.toml"));
        let io = SeedError::Io {
            path: PathBuf::from("data/entries"),
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        };
        assert_eq!(io.exit_code(), IO);
        let thresholds = SeedError::NoThresholds {
            path: PathBuf::from("data/thresholds.toml"),
        };
        assert!(thresholds.to_string().contains("--min-users"));
    }
}
