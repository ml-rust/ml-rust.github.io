//! The errors of `catalogue sandbox` and their exit codes.

use std::io;
use std::path::PathBuf;

use catalogue_core::Diagnostics;
use catalogue_signals::history::HistoryError;
use catalogue_signals::sandbox::SandboxError;
use thiserror::Error;

use crate::cache_dir::CacheDirError;
use crate::data_dir::DataDirError;
use crate::dump::DumpCommandError;
use crate::exit::{INVALID, IO};

/// A `catalogue sandbox` action that fails.
#[derive(Debug, Error)]
pub enum SandboxCommandError {
    /// `GITHUB_ACTIONS` is not `true`.
    #[error(
        "catalogue sandbox check runs third-party build scripts, so it runs only in an ephemeral CI job with no secrets: GITHUB_ACTIONS is not `true`. Run it from the sandbox-check workflow"
    )]
    NotInCi,
    /// `RUNNER_TEMP` is unset or not an absolute path.
    #[error(
        "RUNNER_TEMP is unset or not an absolute path: the check builds on the runner's disk, never in /tmp. Run it from the sandbox-check workflow"
    )]
    NoRunnerTemp,
    /// `--timeout-minutes` is 0.
    #[error("--timeout-minutes 0 is invalid. Pass a value of at least 1")]
    ZeroTimeout,
    /// A list item is not a valid crate release.
    #[error("{} line {line}: {reason}. Write the list again with `catalogue sandbox list`, then rerun", .path.display())]
    InvalidItem {
        /// The list file.
        path: PathBuf,
        /// The 1-based line among the non-blank lines.
        line: usize,
        /// What is wrong.
        reason: String,
    },
    /// The data directory cannot be read.
    #[error(transparent)]
    Data(#[from] DataDirError),
    /// The data files break at least one rule.
    #[error(transparent)]
    Invalid(Diagnostics),
    /// The cache directory cannot be derived.
    #[error(transparent)]
    CacheDir(#[from] CacheDirError),
    /// The cached dump cannot be found or read.
    #[error(transparent)]
    Dump(#[from] DumpCommandError),
    /// The history directory cannot be read.
    #[error(transparent)]
    History(#[from] HistoryError),
    /// A list or result file cannot be read.
    #[error(transparent)]
    File(#[from] SandboxError),
    /// A file or directory cannot be written or removed.
    #[error("cannot write {}: {source}. Check that the directory is writable, then rerun", .path.display())]
    Write {
        /// The file or directory.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// A program cannot start.
    #[error(
        "cannot run `{program}`: {source}. Check that the Rust toolchain is on PATH, then rerun"
    )]
    Spawn {
        /// The program.
        program: &'static str,
        /// The error from the operating system.
        source: io::Error,
    },
}

impl SandboxCommandError {
    /// The process exit code: 1 for refusals and invalid input, 2 for IO.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::NotInCi
            | Self::NoRunnerTemp
            | Self::ZeroTimeout
            | Self::InvalidItem { .. }
            | Self::Invalid(_)
            | Self::File(SandboxError::Corrupt { .. }) => INVALID,
            Self::Dump(error) => error.exit_code(),
            Self::Data(_)
            | Self::CacheDir(_)
            | Self::History(_)
            | Self::File(_)
            | Self::Write { .. }
            | Self::Spawn { .. } => IO,
        }
    }
}
