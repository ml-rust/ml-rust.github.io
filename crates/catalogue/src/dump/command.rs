//! The `catalogue dump` actions and the cached dump reader.

use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_signals::dump::{self, DumpSnapshot};
use catalogue_signals::fetch;
use clap::Subcommand;

use super::error::DumpCommandError;
use super::render::{render_outcome, render_stats};
use crate::cache_dir::resolve_cache_dir;
use crate::usage::usage_dump;

/// Bytes buffered when reading the dump file.
const READ_BUFFER: usize = 1 << 20;

/// The `catalogue dump` actions.
#[derive(Debug, Subcommand)]
pub enum DumpCommand {
    /// Download the crates.io database dump, or resume an interrupted download.
    Fetch {
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
    },
    /// Read the cached dump and print what it holds.
    Stats {
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// Days of download history to keep, ending on the dump date.
        #[arg(long, value_name = "N", default_value_t = 90)]
        window_days: u16,
    },
    /// Read the cached dump and print the adoption signals of each named crate.
    Usage {
        /// The crate names to evaluate.
        #[arg(required = true, value_name = "CRATE")]
        names: Vec<String>,
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// Days of download history to keep, ending on the dump date.
        #[arg(long, value_name = "N", default_value_t = 90)]
        window_days: u16,
    },
}

/// Runs a dump action and returns the process exit code.
pub fn run_dump(command: DumpCommand) -> ExitCode {
    let result = match command {
        DumpCommand::Fetch { cache_dir } => fetch_dump(cache_dir),
        DumpCommand::Stats {
            cache_dir,
            window_days,
        } => stats_dump(cache_dir, window_days),
        DumpCommand::Usage {
            names,
            cache_dir,
            window_days,
        } => usage_dump(&names, cache_dir, window_days),
    };
    match result {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn fetch_dump(cache_dir: Option<PathBuf>) -> Result<String, DumpCommandError> {
    let cache_dir = resolve_cache_dir(cache_dir)?;
    Ok(render_outcome(&fetch::dump(&cache_dir)?))
}

fn stats_dump(cache_dir: Option<PathBuf>, window_days: u16) -> Result<String, DumpCommandError> {
    let cache_dir = resolve_cache_dir(cache_dir)?;
    let snapshot = read_cached(&fetch::dump_path(&cache_dir), window_days)?;
    Ok(render_stats(&snapshot, window_days))
}

/// Reads the dump file at `path`, keeping `window_days` of downloads.
pub(crate) fn read_cached(path: &Path, window_days: u16) -> Result<DumpSnapshot, DumpCommandError> {
    let file = File::open(path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            DumpCommandError::Missing {
                path: path.to_path_buf(),
            }
        } else {
            DumpCommandError::Open {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    dump::read(BufReader::with_capacity(READ_BUFFER, file), window_days).map_err(|source| {
        DumpCommandError::Read {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::error::EXIT_IO;
    use super::*;

    #[test]
    fn missing_dump_names_the_path_and_the_fetch_command() {
        let error = read_cached(Path::new("/nonexistent/db-dump.tar.gz"), 90).expect_err("missing");
        let message = error.to_string();
        assert!(message.contains("/nonexistent/db-dump.tar.gz"), "{message}");
        assert!(message.contains("catalogue dump fetch"), "{message}");
        assert_eq!(error.exit_code(), EXIT_IO);
    }
}
