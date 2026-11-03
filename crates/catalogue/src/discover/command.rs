//! The `catalogue discover` actions and their errors.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_core::{Catalogue, Diagnostics};
use catalogue_signals::discover::Rules;
use catalogue_signals::fetch;
use catalogue_signals::github::GithubError;
use clap::Subcommand;
use thiserror::Error;

use super::github::run_github;
use super::report::{Report, build_report};
use crate::cache_dir::resolve_cache_dir;
use crate::data_dir::{DataDirError, read_data_dir};
use crate::dump::{DumpCommandError, read_cached};
use crate::exit::{INVALID, IO};
use crate::token::NoToken;

/// The `catalogue discover` actions.
#[derive(Debug, Subcommand)]
pub enum DiscoverCommand {
    /// List crates.io crates that match `discovery.toml`, with their adoption signals.
    Crates {
        /// Directory that holds the data files.
        #[arg(long, value_name = "DIR", default_value = "data")]
        data: PathBuf,
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// Days of download history to keep, ending on the dump date.
        #[arg(long, value_name = "N", default_value_t = 90)]
        window_days: u16,
    },
    /// List GitHub repos that match `discovery.toml` and that no crates.io crate links to.
    ///
    /// Reads the token from the `GITHUB_TOKEN` environment variable and the
    /// cached crates.io dump. Run `catalogue dump fetch` first.
    Github {
        /// Directory that holds the data files.
        #[arg(long, value_name = "DIR", default_value = "data")]
        data: PathBuf,
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
    },
}

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
            Self::Invalid(_) | Self::NoRules { .. } | Self::NoTopics { .. } | Self::Token(_) => {
                INVALID
            }
            Self::Dump(error) => error.exit_code(),
            Self::Github(GithubError::Status {
                status: 401 | 422, ..
            }) => INVALID,
            Self::Github(_) => IO,
        }
    }
}

/// Runs a discover action and returns the process exit code.
pub fn run_discover(command: DiscoverCommand) -> ExitCode {
    match command {
        DiscoverCommand::Crates {
            data,
            cache_dir,
            window_days,
        } => run_crates(&data, cache_dir, window_days),
        DiscoverCommand::Github { data, cache_dir } => run_github(&data, cache_dir),
    }
}

/// Prints the outcome of an action that fails and returns its exit code.
pub(super) fn report_error(error: DiscoverError) -> ExitCode {
    if let DiscoverError::Invalid(diagnostics) = &error {
        for diagnostic in diagnostics {
            eprintln!("{diagnostic}");
        }
        return ExitCode::from(INVALID);
    }
    eprintln!("error: {error}");
    ExitCode::from(error.exit_code())
}

fn run_crates(data: &Path, cache_dir: Option<PathBuf>, window_days: u16) -> ExitCode {
    match discover_crates(data, cache_dir, window_days) {
        Ok(report) => {
            for name in &report.unknown_core_crates {
                eprintln!("warning: core crate {name} is not in the dump");
            }
            println!("{}", report.text);
            ExitCode::SUCCESS
        }
        Err(error) => report_error(error),
    }
}

/// Reads and checks the data directory, and returns the catalogue.
pub(super) fn load_catalogue(data: &Path) -> Result<Catalogue, DiscoverError> {
    let dir = read_data_dir(data)?;
    let mut diagnostics = dir.diagnostics;
    match Catalogue::load(dir.sources) {
        Ok(catalogue) if diagnostics.is_empty() => Ok(catalogue),
        Ok(_) => Err(DiscoverError::Invalid(diagnostics)),
        Err(found) => {
            diagnostics.extend(found);
            Err(DiscoverError::Invalid(diagnostics))
        }
    }
}

/// Checks the data, reads the cached dump and reports the candidates.
fn discover_crates(
    data: &Path,
    cache_dir: Option<PathBuf>,
    window_days: u16,
) -> Result<Report, DiscoverError> {
    let catalogue = load_catalogue(data)?;
    let Some(discovery) = catalogue.discovery() else {
        return Err(DiscoverError::NoRules {
            path: data.join("discovery.toml"),
        });
    };
    let rules = Rules {
        categories: discovery.categories.clone(),
        keywords: discovery.keywords.clone(),
        core_crates: discovery
            .core_crates
            .iter()
            .map(ToString::to_string)
            .collect(),
    };
    let mut listed: HashMap<String, &str> = HashMap::new();
    for (slug, entry) in catalogue.entries() {
        for name in &entry.crates {
            listed.insert(name.normalized(), slug.as_str());
        }
    }
    let cache_dir = resolve_cache_dir(cache_dir).map_err(DumpCommandError::from)?;
    let snapshot = read_cached(&fetch::dump_path(&cache_dir), window_days)?;
    Ok(build_report(&snapshot, &rules, &listed))
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
