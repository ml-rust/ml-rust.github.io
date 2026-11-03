//! The errors that stop a bot run, and those that stop one issue.

use std::io;
use std::path::PathBuf;

use catalogue_github::ApiError;
use thiserror::Error;

use crate::bot::labels::LabelsCommandError;
use crate::cache_dir::CacheDirError;
use crate::exit::{INVALID, IO};

/// A bot run that stops before it checks every issue.
#[derive(Debug, Error)]
pub enum RunError {
    /// The config, the labels file or the key cannot be used, or the label sync fails.
    #[error(transparent)]
    Setup(#[from] LabelsCommandError),
    /// No state directory can be derived.
    #[error(
        "cannot derive the bot state directory: state_dir is unset in the bot config, STATE_DIRECTORY is unset, and XDG_CACHE_HOME and HOME give no cache directory. Set state_dir in the bot config"
    )]
    StateDir(#[from] CacheDirError),
    /// Another bot run holds the lock.
    #[error(
        "another bot run holds the lock {}. Wait for it to finish, then rerun",
        .path.display()
    )]
    Locked {
        /// The lock file.
        path: PathBuf,
    },
    /// The lock file cannot be opened or locked.
    #[error(
        "cannot take the lock {}: {source}. Check that the state directory is writable, then rerun",
        .path.display()
    )]
    Lock {
        /// The lock file or the state directory.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// A fetch the whole run needs fails: the data files, the tier snapshot or the issue list.
    #[error(transparent)]
    Api(#[from] ApiError),
}

impl RunError {
    /// The process exit code: 1 for a config, labels file or key to fix, 2 otherwise.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Setup(error) => error.exit_code(),
            Self::StateDir(_) => INVALID,
            Self::Locked { .. } | Self::Lock { .. } | Self::Api(_) => IO,
        }
    }
}

/// An issue the run skips. The message holds the issue number and no issue text.
#[derive(Debug, Error)]
pub enum IssueError {
    /// The API sends no valid author login for the issue.
    #[error(
        "issue {number}: the API sends no valid author login. The issue is skipped. Check the issue author on GitHub"
    )]
    Author {
        /// The issue number.
        number: u64,
    },
    /// A GitHub call for the issue fails.
    #[error("issue {number}: {source}")]
    Api {
        /// The issue number.
        number: u64,
        /// The failed call.
        source: ApiError,
    },
}
