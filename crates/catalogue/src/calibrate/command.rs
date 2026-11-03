//! The `catalogue calibrate` actions and their errors.

use std::path::PathBuf;
use std::process::ExitCode;

use catalogue_signals::distribution::DistributionError;
use clap::Subcommand;
use thiserror::Error;

use super::apps::calibrate_apps;
use super::crates::calibrate_crates;
use crate::discover::{DiscoverError, report_error};
use crate::dump::DumpCommandError;
use crate::exit::{INVALID, IO};
use crate::token::{NoToken, token_from_env};

/// The percentile printed on the `at` line when `--percentile` is absent.
const DEFAULT_PERCENTILE: f64 = 90.0;

/// The `catalogue calibrate` actions.
///
/// Each action prints one block per signal: the signal name, the population
/// size, the 50th, 75th, 90th, 95th and 99th percentiles, and the value at
/// `--percentile`. Percentiles use the nearest-rank definition: rank
/// `ceil(p / 100 * n)` of `n` sorted values, counted from 1.
#[derive(Debug, Subcommand)]
pub enum CalibrateCommand {
    /// Print percentiles of the crate signals over the discovery candidates.
    ///
    /// The population is the crates.io discovery candidates with at least one
    /// active user. Reads the cached dump. Run `catalogue dump fetch` first.
    Crates {
        /// Directory that holds the data files.
        #[arg(long, value_name = "DIR", default_value = "data")]
        data: PathBuf,
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// The percentile to print on the `at` line, above 0 and up to 100.
        #[arg(
            long,
            value_name = "P",
            default_value_t = DEFAULT_PERCENTILE,
            value_parser = parse_percentile
        )]
        percentile: f64,
    },
    /// Print percentiles of the binary downloads of app repos.
    ///
    /// Takes the leads of `discover github`, fetches the releases of the
    /// first `--limit` leads by stars, and sums the binary downloads of the
    /// latest 3 releases. Reads the token from the `GITHUB_TOKEN` environment
    /// variable and the cached dump. A release fetch error other than a
    /// rejected token skips the repo and shows in the skipped count.
    Apps {
        /// Directory that holds the data files.
        #[arg(long, value_name = "DIR", default_value = "data")]
        data: PathBuf,
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// The percentile to print on the `at` line, above 0 and up to 100.
        #[arg(
            long,
            value_name = "P",
            default_value_t = DEFAULT_PERCENTILE,
            value_parser = parse_percentile
        )]
        percentile: f64,
        /// The most leads to fetch, taken by stars descending. Default: all.
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
    },
}

/// A `catalogue calibrate` action that fails.
#[derive(Debug, Error)]
pub enum CalibrateError {
    /// The data, the rules, the token or the GitHub search fails.
    #[error(transparent)]
    Discover(#[from] DiscoverError),
    /// The cached dump cannot be found or read.
    #[error(transparent)]
    Dump(#[from] DumpCommandError),
    /// A release request fails in a way that stops the run.
    #[error(transparent)]
    Distribution(#[from] DistributionError),
}

impl From<NoToken> for CalibrateError {
    fn from(error: NoToken) -> Self {
        Self::Discover(DiscoverError::Token(error))
    }
}

impl CalibrateError {
    /// The process exit code.
    ///
    /// 1 for data, rules, a missing token and a rejected token. 2 for IO and
    /// network errors. The dump code for dump errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Discover(error) => error.exit_code(),
            Self::Dump(error) => error.exit_code(),
            Self::Distribution(DistributionError::Status { status: 401, .. }) => INVALID,
            Self::Distribution(_) => IO,
        }
    }
}

/// Runs a calibrate action and returns the process exit code.
pub fn run_calibrate(command: CalibrateCommand) -> ExitCode {
    match run(command) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(CalibrateError::Discover(error)) => report_error(error),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

/// Runs an action and renders its report.
fn run(command: CalibrateCommand) -> Result<String, CalibrateError> {
    match command {
        CalibrateCommand::Crates {
            data,
            cache_dir,
            percentile,
        } => calibrate_crates(&data, cache_dir, percentile),
        CalibrateCommand::Apps {
            data,
            cache_dir,
            percentile,
            limit,
        } => calibrate_apps(&data, cache_dir, percentile, limit, token_from_env()),
    }
}

/// Parses a percentile above 0 and up to 100.
fn parse_percentile(text: &str) -> Result<f64, String> {
    let value: f64 = text.trim().parse().map_err(|_| {
        format!("`{text}` is not a number. Pass a percentile above 0 and up to 100, for example 90")
    })?;
    if value.is_finite() && value > 0.0 && value <= 100.0 {
        Ok(value)
    } else {
        Err(format!(
            "`{text}` is out of range. Pass a percentile above 0 and up to 100, for example 90"
        ))
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    #[test]
    fn a_percentile_is_above_zero_and_at_most_100() {
        assert_eq!(parse_percentile("90"), Ok(90.0));
        assert_eq!(parse_percentile(" 99.5 "), Ok(99.5));
        assert_eq!(parse_percentile("100"), Ok(100.0));
        for bad in ["0", "-1", "100.1", "nan", "inf", "ninety", ""] {
            assert!(parse_percentile(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn errors_carry_their_exit_codes() {
        assert_eq!(CalibrateError::from(NoToken).exit_code(), INVALID);
        let status = |status| {
            CalibrateError::Distribution(DistributionError::Status {
                api: "GitHub Releases",
                url: "u".to_owned(),
                status,
                action: "x",
            })
        };
        assert_eq!(status(401).exit_code(), INVALID);
        assert_eq!(status(500).exit_code(), IO);
    }

    #[test]
    fn crates_has_defaults_and_takes_options() {
        let cli = Cli::try_parse_from(["catalogue", "calibrate", "crates"]).expect("parses");
        let Command::Calibrate {
            command:
                CalibrateCommand::Crates {
                    data,
                    cache_dir,
                    percentile,
                },
        } = cli.command
        else {
            panic!("expected calibrate crates");
        };
        assert_eq!(
            (data, cache_dir, percentile),
            (PathBuf::from("data"), None, 90.0)
        );
        let cli = Cli::try_parse_from([
            "catalogue",
            "calibrate",
            "crates",
            "--percentile",
            "75",
            "--cache-dir",
            "c",
        ])
        .expect("parses");
        let Command::Calibrate {
            command:
                CalibrateCommand::Crates {
                    cache_dir,
                    percentile,
                    ..
                },
        } = cli.command
        else {
            panic!("expected calibrate crates");
        };
        assert_eq!((cache_dir, percentile), (Some(PathBuf::from("c")), 75.0));
    }

    #[test]
    fn apps_takes_a_limit_and_rejects_a_bad_percentile() {
        let cli = Cli::try_parse_from(["catalogue", "calibrate", "apps", "--limit", "20"])
            .expect("parses");
        let Command::Calibrate {
            command: CalibrateCommand::Apps {
                limit, percentile, ..
            },
        } = cli.command
        else {
            panic!("expected calibrate apps");
        };
        assert_eq!((limit, percentile), (Some(20), 90.0));
        assert!(
            Cli::try_parse_from(["catalogue", "calibrate", "apps", "--percentile", "0"]).is_err()
        );
    }
}
