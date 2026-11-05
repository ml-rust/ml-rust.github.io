//! The errors of `catalogue tiers`.

use std::io;
use std::path::PathBuf;

use catalogue_core::Diagnostics;
use catalogue_signals::distribution::DistributionError;
use catalogue_signals::fallback::FetchSummary;
use catalogue_signals::github::GithubError;
use catalogue_signals::history::HistoryError;
use catalogue_signals::http::HttpError;
use catalogue_signals::link::LinkError;
use catalogue_signals::sandbox::SandboxError;
use thiserror::Error;

use crate::cache_dir::CacheDirError;
use crate::data_dir::DataDirError;
use crate::dump::DumpCommandError;
use crate::exit::{INVALID, IO};
use crate::rustsec::RustsecCommandError;
use crate::token::NoToken;

/// A `catalogue tiers` run that fails.
#[derive(Debug, Error)]
pub enum TiersError {
    /// The data directory cannot be read.
    #[error(transparent)]
    Data(#[from] DataDirError),
    /// The data files break at least one rule.
    #[error(transparent)]
    Invalid(Diagnostics),
    /// A data file the tier rules need is absent.
    #[error("no {name} in the data directory. Create the file, then rerun")]
    MissingFile {
        /// The file name.
        name: &'static str,
    },
    /// The cache directory cannot be derived.
    #[error(transparent)]
    CacheDir(#[from] CacheDirError),
    /// The cached dump cannot be found or read.
    #[error(transparent)]
    Dump(#[from] DumpCommandError),
    /// The cached advisory archive cannot be found or read.
    #[error(transparent)]
    Rustsec(#[from] RustsecCommandError),
    /// `GITHUB_TOKEN` is unset or empty.
    #[error(transparent)]
    Token(#[from] NoToken),
    /// The HTTP client cannot start.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// A GitHub facts request fails.
    #[error(transparent)]
    Github(#[from] GithubError),
    /// A release, Homebrew or Repology request fails.
    #[error(transparent)]
    Distribution(#[from] DistributionError),
    /// A repo link check fails.
    #[error(transparent)]
    Link(#[from] LinkError),
    /// The history directory cannot be read or written.
    #[error(transparent)]
    History(#[from] HistoryError),
    /// The `--build-checks` file cannot be read.
    #[error(transparent)]
    BuildChecks(#[from] SandboxError),
    /// The `--json` report cannot be written.
    #[error(
        "cannot write the tier report {}: {source}. Check that the --json path is a writable file path, then rerun",
        .path.display()
    )]
    Report {
        /// The file or directory that fails.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// A signal in the report has a value without a source link or fetch date.
    #[error(
        "the tier report cites no source or date for some signals, so it is not written: {}. Fix the fetch that produced them, then rerun",
        .problems.join("; ")
    )]
    Uncited {
        /// One line per problem, naming the entry and the signal.
        problems: Vec<String>,
    },
    /// Too many fetches failed with no usable earlier value.
    #[error(
        "{} of {} network fetches failed with no usable earlier value, above the limit of {limit} in fallback.max_failed_share. The run wrote no snapshot and no report:\n{}\nFix these fetch errors, then rerun",
        .summary.lost_count(),
        .summary.fetches,
        .summary.lost_lines()
    )]
    TooManyFailed {
        /// The limit from `data/thresholds.toml`.
        limit: f64,
        /// The fetches of the run.
        summary: FetchSummary,
    },
}

impl TiersError {
    /// The process exit code.
    ///
    /// 1 for data, a missing data file, a missing token, a rejected token and a
    /// corrupt build check file. 2 for IO, network and report write errors, and for
    /// too many failed fetches. The dump and archive codes for their errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Invalid(_)
            | Self::MissingFile { .. }
            | Self::Token(_)
            | Self::Github(GithubError::Status { status: 401, .. })
            | Self::Link(LinkError::Github(GithubError::Status { status: 401, .. }))
            | Self::Distribution(DistributionError::Status { status: 401, .. })
            | Self::BuildChecks(SandboxError::Corrupt { .. })
            | Self::Uncited { .. } => INVALID,
            Self::Dump(error) => error.exit_code(),
            Self::Rustsec(error) => error.exit_code(),
            Self::Data(_)
            | Self::CacheDir(_)
            | Self::Http(_)
            | Self::Github(_)
            | Self::Link(_)
            | Self::History(_)
            | Self::BuildChecks(_)
            | Self::Distribution(_)
            | Self::Report { .. }
            | Self::TooManyFailed { .. } => IO,
        }
    }
}
