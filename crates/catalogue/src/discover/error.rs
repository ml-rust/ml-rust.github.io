//! The errors of the `catalogue discover` actions.

use std::path::PathBuf;

use catalogue_core::Diagnostics;
use catalogue_signals::github::GithubError;
use thiserror::Error;

use crate::data_dir::DataDirError;
use crate::dump::DumpCommandError;
use crate::exit::{INVALID, IO};
use crate::seed::SeedError;
use crate::token::NoToken;

/// A `catalogue discover` action that fails.
#[derive(Debug, Error)]
pub enum DiscoverError {
    /// The data directory cannot be read.
    #[error(transparent)]
    Data(#[from] DataDirError),
    /// The data files break at least one rule.
    #[error(transparent)]
    Invalid(Diagnostics),
    /// The data directory has no `discovery.toml`.
    #[error("no discovery rules at {}. Create the file, then rerun", .path.display())]
    NoRules {
        /// The expected rules file.
        path: PathBuf,
    },
    /// `discovery.toml` lists no GitHub topic.
    #[error("no github_topics in {}. Add at least one topic, then rerun", .path.display())]
    NoTopics {
        /// The rules file.
        path: PathBuf,
    },
    /// `discovery.toml` has no `[below_floor]` table.
    #[error("no [below_floor] table in {}. Add min_stars and active_days, then rerun", .path.display())]
    NoBelowFloor {
        /// The rules file.
        path: PathBuf,
    },
    /// The lead file cannot be read.
    #[error("cannot read the lead file {}: {source}. Make it readable, then rerun", .path.display())]
    ReadLeads {
        /// The lead file.
        path: PathBuf,
        /// The read error.
        source: std::io::Error,
    },
    /// A line of the lead file is not a repo.
    #[error(transparent)]
    Leads(Box<SeedError>),
    /// `GITHUB_TOKEN` is unset or empty.
    #[error(transparent)]
    Token(#[from] NoToken),
    /// The cached dump cannot be found or read.
    #[error(transparent)]
    Dump(#[from] DumpCommandError),
    /// The GitHub search fails.
    #[error(transparent)]
    Github(#[from] GithubError),
}

impl DiscoverError {
    /// The process exit code.
    ///
    /// 1 for data, rules, a missing token and a rejected token or query. 2 for
    /// IO and network errors. The dump code for dump errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Data(_) => IO,
            Self::Invalid(_)
            | Self::NoRules { .. }
            | Self::NoTopics { .. }
            | Self::NoBelowFloor { .. }
            | Self::Token(_) => INVALID,
            Self::ReadLeads { .. } => IO,
            Self::Leads(error) => error.exit_code(),
            Self::Dump(error) => error.exit_code(),
            Self::Github(GithubError::Status {
                status: 401 | 422, ..
            }) => INVALID,
            Self::Github(_) => IO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_carry_their_exit_codes() {
        let rules = DiscoverError::NoRules {
            path: PathBuf::from("data/discovery.toml"),
        };
        assert_eq!(rules.exit_code(), INVALID);
        assert!(rules.to_string().contains("data/discovery.toml"));
        let invalid = DiscoverError::Invalid(Diagnostics::new());
        assert_eq!(invalid.exit_code(), INVALID);
        let missing = DiscoverError::Data(DataDirError::Missing {
            path: PathBuf::from("data"),
        });
        assert_eq!(missing.exit_code(), IO);
    }

    #[test]
    fn github_errors_carry_their_exit_codes() {
        let topics = DiscoverError::NoTopics {
            path: PathBuf::from("data/discovery.toml"),
        };
        assert_eq!(topics.exit_code(), INVALID);
        assert!(topics.to_string().contains("discovery.toml"));
        assert_eq!(DiscoverError::Token(NoToken).exit_code(), INVALID);
        let message = DiscoverError::Token(NoToken).to_string();
        assert!(message.contains("set GITHUB_TOKEN"), "{message}");
        assert!(message.contains("gh auth token"), "{message}");
        let status = |status| {
            DiscoverError::Github(GithubError::Status {
                url: "u".to_owned(),
                status,
                action: "x",
            })
        };
        assert_eq!(status(401).exit_code(), INVALID);
        assert_eq!(status(500).exit_code(), IO);
    }
}
