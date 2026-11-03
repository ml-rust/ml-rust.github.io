//! The errors of `catalogue build`.

use std::io;
use std::path::PathBuf;

use catalogue_core::Diagnostics;
use catalogue_signals::history::HistoryError;
use catalogue_site::SiteError;
use thiserror::Error;

use crate::data_dir::DataDirError;
use crate::exit::{INVALID, IO};

/// A `catalogue build` run that fails.
#[derive(Debug, Error)]
pub enum BuildError {
    /// The data directory cannot be read.
    #[error(transparent)]
    Data(#[from] DataDirError),
    /// The data files break at least one rule.
    #[error(transparent)]
    Invalid(Diagnostics),
    /// The report file cannot be read.
    #[error(
        "cannot read the tier report {}: {source}. Check the --report path, or run `catalogue tiers --json FILE` first",
        .path.display()
    )]
    ReportRead {
        /// The report file.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// The report file is not a tier report.
    #[error(
        "the tier report {} is not valid: {message}. Rerun `catalogue tiers --json {}`",
        .path.display(), .path.display()
    )]
    ReportParse {
        /// The report file.
        path: PathBuf,
        /// What the JSON parser reports.
        message: String,
    },
    /// The history directory cannot be read.
    #[error(transparent)]
    History(#[from] HistoryError),
    /// A history row holds a slug that is not a slug.
    #[error(
        "the history holds the invalid slug {slug:?}. Remove or repair that snapshot in the --history directory, then rerun"
    )]
    HistorySlug {
        /// The slug text.
        slug: String,
    },
    /// The site build refuses its inputs.
    #[error(transparent)]
    Site(#[from] SiteError),
    /// The site build returned a path that leaves the output directory.
    #[error(
        "the site build produced the unsafe path {path:?}, so nothing is written. This is a bug in catalogue-site: report it"
    )]
    UnsafePath {
        /// The path as the site build gave it.
        path: String,
    },
    /// The swap failed and the previous site could not be moved back.
    #[error(
        "cannot restore the previous site to {}: {source}. It is at {}: move it back by hand",
        .out.display(), .old.display()
    )]
    Rollback {
        /// The output directory.
        out: PathBuf,
        /// Where the previous site is now.
        old: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// The site build returned one path twice.
    #[error(
        "the site build produced the path {path:?} twice, so nothing is written. This is a bug in catalogue-site: report it"
    )]
    DuplicatePath {
        /// The repeated path.
        path: String,
    },
    /// The output path cannot be replaced.
    #[error("cannot replace {}: {reason}. Choose another --out directory, then rerun", .path.display())]
    Out {
        /// The --out path.
        path: PathBuf,
        /// Why the path is refused.
        reason: String,
    },
    /// A write, rename or removal fails.
    #[error("{}: {source}. Check the --out path and its permissions, then rerun", .path.display())]
    Write {
        /// The file or directory that fails.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
}

impl BuildError {
    /// The process exit code: 1 for data, report, history content and site
    /// refusals, 2 for an IO error.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Invalid(_)
            | Self::ReportParse { .. }
            | Self::HistorySlug { .. }
            | Self::Site(_)
            | Self::UnsafePath { .. }
            | Self::DuplicatePath { .. }
            | Self::Out { .. } => INVALID,
            Self::Data(_)
            | Self::ReportRead { .. }
            | Self::History(_)
            | Self::Write { .. }
            | Self::Rollback { .. } => IO,
        }
    }
}
