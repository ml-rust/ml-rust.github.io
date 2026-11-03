//! The `catalogue discover` actions and their errors.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_core::{Catalogue, Diagnostics};
use catalogue_signals::discover::Rules;
use catalogue_signals::fetch;
use clap::Subcommand;
use thiserror::Error;

use super::report::{Report, build_report};
use crate::cache_dir::resolve_cache_dir;
use crate::data_dir::{DataDirError, read_data_dir};
use crate::dump::{DumpCommandError, read_cached};
use crate::exit::{INVALID, IO};

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
    /// The cached dump cannot be found or read.
    #[error(transparent)]
    Dump(#[from] DumpCommandError),
}

impl DiscoverError {
    /// The process exit code: 1 for data and rules, 2 for IO, the dump code for dump errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Data(_) => IO,
            Self::Invalid(_) | Self::NoRules { .. } => INVALID,
            Self::Dump(error) => error.exit_code(),
        }
    }
}

/// Runs a discover action and returns the process exit code.
pub fn run_discover(command: DiscoverCommand) -> ExitCode {
    let DiscoverCommand::Crates {
        data,
        cache_dir,
        window_days,
    } = command;
    match discover_crates(&data, cache_dir, window_days) {
        Ok(report) => {
            for name in &report.unknown_core_crates {
                eprintln!("warning: core crate {name} is not in the dump");
            }
            println!("{}", report.text);
            ExitCode::SUCCESS
        }
        Err(DiscoverError::Invalid(diagnostics)) => {
            for diagnostic in &diagnostics {
                eprintln!("{diagnostic}");
            }
            ExitCode::from(INVALID)
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

/// Checks the data, reads the cached dump and reports the candidates.
fn discover_crates(
    data: &Path,
    cache_dir: Option<PathBuf>,
    window_days: u16,
) -> Result<Report, DiscoverError> {
    let dir = read_data_dir(data)?;
    let mut diagnostics = dir.diagnostics;
    let catalogue = match Catalogue::load(dir.sources) {
        Ok(catalogue) if diagnostics.is_empty() => catalogue,
        Ok(_) => return Err(DiscoverError::Invalid(diagnostics)),
        Err(found) => {
            diagnostics.extend(found);
            return Err(DiscoverError::Invalid(diagnostics));
        }
    };
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
}
