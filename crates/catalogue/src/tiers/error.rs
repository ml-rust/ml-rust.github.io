//! The errors of `catalogue tiers`.

use catalogue_core::Diagnostics;
use catalogue_signals::distribution::DistributionError;
use catalogue_signals::github::GithubError;
use catalogue_signals::history::HistoryError;
use catalogue_signals::http::HttpError;
use catalogue_signals::link::LinkError;
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
}

impl TiersError {
    /// The process exit code.
    ///
    /// 1 for data, a missing data file, a missing token and a rejected token. 2
    /// for IO and network errors. The dump and archive codes for their errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Invalid(_)
            | Self::MissingFile { .. }
            | Self::Token(_)
            | Self::Github(GithubError::Status { status: 401, .. })
            | Self::Link(LinkError::Github(GithubError::Status { status: 401, .. }))
            | Self::Distribution(DistributionError::Status { status: 401, .. }) => INVALID,
            Self::Dump(error) => error.exit_code(),
            Self::Rustsec(error) => error.exit_code(),
            Self::Data(_)
            | Self::CacheDir(_)
            | Self::Http(_)
            | Self::Github(_)
            | Self::Link(_)
            | Self::History(_)
            | Self::Distribution(_) => IO,
        }
    }
}
