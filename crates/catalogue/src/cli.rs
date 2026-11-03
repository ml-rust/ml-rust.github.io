use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::bot::{BotCommand, run_bot};
use crate::calibrate::{CalibrateCommand, run_calibrate};
use crate::discover::{DiscoverCommand, run_discover};
use crate::distribution::{DistributionCommand, run_distribution};
use crate::dump::{DumpCommand, run_dump};
use crate::ecosystems::{EcosystemsCommand, run_ecosystems};
use crate::exit::{INVALID, IO};
use crate::github::{GithubCommand, run_github};
use crate::rustsec::{RustsecCommand, run_rustsec};
use crate::seed::{SeedArgs, run_seed};
use crate::tiers::{TiersArgs, run_tiers};
use crate::validate::{Outcome, validate};

/// Checks the ml-rust catalogue data files.
#[derive(Debug, Parser)]
#[command(name = "catalogue", version)]
pub struct Cli {
    /// The action to run.
    #[command(subcommand)]
    pub command: Command,
}

/// The actions of the `catalogue` command.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check every data file and every rule across files.
    Validate {
        /// Directory that holds the data files.
        #[arg(long, value_name = "DIR", default_value = "data")]
        data: PathBuf,
    },
    /// Download and inspect the crates.io database dump.
    Dump {
        /// The dump action to run.
        #[command(subcommand)]
        command: DumpCommand,
    },
    /// Find candidate crates for the catalogue in the crates.io database dump.
    Discover {
        /// The discover action to run.
        #[command(subcommand)]
        command: DiscoverCommand,
    },
    /// Read release, Homebrew, Docker Hub and Repology counts for apps.
    Distribution {
        /// The distribution action to run.
        #[command(subcommand)]
        command: DistributionCommand,
    },
    /// Read dependent counts from ecosyste.ms.
    Ecosystems {
        /// The ecosystems action to run.
        #[command(subcommand)]
        command: EcosystemsCommand,
    },
    /// Read facts about GitHub repos.
    Github {
        /// The github action to run.
        #[command(subcommand)]
        command: GithubCommand,
    },
    /// Download and inspect the RustSec advisory database.
    Rustsec {
        /// The rustsec action to run.
        #[command(subcommand)]
        command: RustsecCommand,
    },
    /// Print percentiles of the tier signals, for setting thresholds.
    Calibrate {
        /// The calibrate action to run.
        #[command(subcommand)]
        command: CalibrateCommand,
    },
    /// Assign each entry its tier and print the reasons.
    ///
    /// Reads the cached crates.io dump and RustSec archive. Unless `--offline`,
    /// reads the token from the `GITHUB_TOKEN` environment variable and fetches
    /// repo facts and app distribution counts.
    Tiers(TiersArgs),
    /// Draft entries from discovery for human review.
    ///
    /// Drafts one library entry per crate that matches `discovery.toml`, has
    /// enough active users, is not listed yet, and maps to a topic in
    /// `topics.toml`. A reviewer merges crate families and checks every draft.
    /// Without `--write` the command prints the drafts and the skipped crates.
    Seed(SeedArgs),
    /// Check issue forms, sync repo labels and run the issue checks of the catalogue bot.
    Bot {
        /// The bot action to run.
        #[command(subcommand)]
        command: BotCommand,
    },
}

/// Runs the parsed command and returns the process exit code.
///
/// The code is 0 for success, 1 for data, dump or archive errors and 2 for an IO or network error.
pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Command::Validate { data } => run_validate(&data),
        Command::Dump { command } => run_dump(command),
        Command::Discover { command } => run_discover(command),
        Command::Distribution { command } => run_distribution(command),
        Command::Ecosystems { command } => run_ecosystems(command),
        Command::Github { command } => run_github(command),
        Command::Rustsec { command } => run_rustsec(command),
        Command::Calibrate { command } => run_calibrate(command),
        Command::Tiers(args) => run_tiers(args),
        Command::Seed(args) => run_seed(args),
        Command::Bot { command } => run_bot(command),
    }
}

fn run_validate(data: &Path) -> ExitCode {
    match validate(data) {
        Ok(Outcome::Valid(summary)) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Ok(Outcome::Invalid(diagnostics)) => {
            for diagnostic in &diagnostics {
                eprintln!("{diagnostic}");
            }
            ExitCode::from(INVALID)
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(IO)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_defaults_to_the_data_directory() {
        let cli = Cli::try_parse_from(["catalogue", "validate"]).expect("parses");
        let Command::Validate { data } = cli.command else {
            panic!("expected validate");
        };
        assert_eq!(data, PathBuf::from("data"));
    }

    #[test]
    fn validate_takes_a_data_directory() {
        let cli =
            Cli::try_parse_from(["catalogue", "validate", "--data", "other"]).expect("parses");
        let Command::Validate { data } = cli.command else {
            panic!("expected validate");
        };
        assert_eq!(data, PathBuf::from("other"));
    }

    #[test]
    fn a_subcommand_is_required() {
        assert!(Cli::try_parse_from(["catalogue"]).is_err());
    }
}
