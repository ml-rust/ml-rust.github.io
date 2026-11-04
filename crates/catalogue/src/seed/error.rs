//! The error of `catalogue seed` and its exit codes.

use std::io;
use std::path::PathBuf;

use catalogue_signals::github::GithubError;
use thiserror::Error;

use crate::discover::DiscoverError;
use crate::dump::DumpCommandError;
use crate::exit::{INVALID, IO};
use crate::token::NoToken;

/// A `catalogue seed` run that fails.
#[derive(Debug, Error)]
pub enum SeedError {
    /// The data files, the discovery rules or the cache directory break a rule.
    #[error(transparent)]
    Discover(#[from] DiscoverError),
    /// The cached dump cannot be found or read.
    #[error(transparent)]
    Dump(#[from] DumpCommandError),
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
    /// The lead file cannot be read.
    #[error("cannot read the lead file {}: {source}. Pass a readable file to --leads, or --no-leads for no list", .path.display())]
    ReadLeads {
        /// The lead file.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// A line of the lead file is not a repo.
    #[error("{}:{line}: `{input}` is not a repo in the form owner/name. Fix the line, then rerun", .path.display())]
    BadLead {
        /// The lead file.
        path: PathBuf,
        /// The line number, from 1.
        line: usize,
        /// The rejected text.
        input: String,
    },
    /// The lead file names no repo.
    #[error("the lead file {} names no repo. Add one owner/name per line, then rerun", .path.display())]
    NoLeads {
        /// The lead file.
        path: PathBuf,
    },
    /// `GITHUB_TOKEN` is unset or empty.
    #[error(transparent)]
    Token(#[from] NoToken),
    /// GitHub rejects the token, or the HTTP client cannot start.
    #[error(transparent)]
    Github(#[from] GithubError),
}

impl SeedError {
    /// The process exit code.
    ///
    /// 1 for rule breaks, an existing entry file, a bad lead file, a
    /// missing token and a rejected token. 2 for IO and network errors. The inner code for
    /// discover and dump errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Discover(error) => error.exit_code(),
            Self::Dump(error) => error.exit_code(),
            Self::Exists { .. }
            | Self::Serialize { .. }
            | Self::BadLead { .. }
            | Self::NoLeads { .. }
            | Self::Token(_)
            | Self::Github(GithubError::Status { status: 401, .. }) => INVALID,
            Self::Io { .. } | Self::ReadLeads { .. } | Self::Github(_) => IO,
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
    }

    #[test]
    fn lead_errors_name_the_input_and_the_next_action() {
        let bad = SeedError::BadLead {
            path: PathBuf::from("leads.txt"),
            line: 3,
            input: "nope".to_owned(),
        };
        assert_eq!(bad.exit_code(), INVALID);
        assert_eq!(
            bad.to_string(),
            "leads.txt:3: `nope` is not a repo in the form owner/name. Fix the line, then rerun"
        );
        let read = SeedError::ReadLeads {
            path: PathBuf::from("leads.txt"),
            source: io::Error::from(io::ErrorKind::NotFound),
        };
        assert_eq!(read.exit_code(), IO);
        assert!(read.to_string().contains("--leads"), "{read}");
        assert_eq!(SeedError::Token(NoToken).exit_code(), INVALID);
    }

    #[test]
    fn a_rejected_token_exits_one_and_other_github_errors_exit_two() {
        let status = |status| {
            SeedError::Github(GithubError::Status {
                url: "u".to_owned(),
                status,
                action: "a",
            })
        };
        assert_eq!(status(401).exit_code(), INVALID);
        assert_eq!(status(500).exit_code(), IO);
    }
}
