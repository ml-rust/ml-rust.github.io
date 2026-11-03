use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::discover::{DiscoverCommand, run_discover};
use crate::distribution::{DistributionCommand, run_distribution};
use crate::dump::{DumpCommand, run_dump};
use crate::ecosystems::{EcosystemsCommand, run_ecosystems};
use crate::exit::{INVALID, IO};
use crate::github::{GithubCommand, run_github};
use crate::rustsec::{RustsecCommand, run_rustsec};
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
    fn dump_fetch_takes_an_optional_cache_dir() {
        let cli = Cli::try_parse_from(["catalogue", "dump", "fetch"]).expect("parses");
        let Command::Dump {
            command: DumpCommand::Fetch { cache_dir },
        } = cli.command
        else {
            panic!("expected dump fetch");
        };
        assert_eq!(cache_dir, None);
        let cli = Cli::try_parse_from(["catalogue", "dump", "fetch", "--cache-dir", "c"])
            .expect("parses");
        let Command::Dump {
            command: DumpCommand::Fetch { cache_dir },
        } = cli.command
        else {
            panic!("expected dump fetch");
        };
        assert_eq!(cache_dir, Some(PathBuf::from("c")));
    }

    #[test]
    fn dump_stats_window_defaults_to_90_days() {
        let cli = Cli::try_parse_from(["catalogue", "dump", "stats"]).expect("parses");
        let Command::Dump {
            command: DumpCommand::Stats { window_days, .. },
        } = cli.command
        else {
            panic!("expected dump stats");
        };
        assert_eq!(window_days, 90);
        let cli = Cli::try_parse_from(["catalogue", "dump", "stats", "--window-days", "30"])
            .expect("parses");
        let Command::Dump {
            command: DumpCommand::Stats { window_days, .. },
        } = cli.command
        else {
            panic!("expected dump stats");
        };
        assert_eq!(window_days, 30);
    }

    #[test]
    fn dump_usage_takes_crate_names() {
        let cli =
            Cli::try_parse_from(["catalogue", "dump", "usage", "numr", "blazr"]).expect("parses");
        let Command::Dump {
            command:
                DumpCommand::Usage {
                    names,
                    cache_dir,
                    window_days,
                },
        } = cli.command
        else {
            panic!("expected dump usage");
        };
        assert_eq!(names, ["numr", "blazr"]);
        assert_eq!((cache_dir, window_days), (None, 90));
        assert!(Cli::try_parse_from(["catalogue", "dump", "usage"]).is_err());
    }

    #[test]
    fn discover_crates_has_defaults_and_takes_options() {
        let cli = Cli::try_parse_from(["catalogue", "discover", "crates"]).expect("parses");
        let Command::Discover {
            command:
                DiscoverCommand::Crates {
                    data,
                    cache_dir,
                    window_days,
                },
        } = cli.command
        else {
            panic!("expected discover crates");
        };
        assert_eq!(
            (data, cache_dir, window_days),
            (PathBuf::from("data"), None, 90)
        );
        let cli = Cli::try_parse_from([
            "catalogue",
            "discover",
            "crates",
            "--data",
            "d",
            "--cache-dir",
            "c",
            "--window-days",
            "30",
        ])
        .expect("parses");
        let Command::Discover {
            command:
                DiscoverCommand::Crates {
                    data,
                    cache_dir,
                    window_days,
                },
        } = cli.command
        else {
            panic!("expected discover crates");
        };
        assert_eq!(
            (data, cache_dir, window_days),
            (PathBuf::from("d"), Some(PathBuf::from("c")), 30)
        );
    }

    #[test]
    fn discover_github_has_defaults_and_takes_options() {
        let cli = Cli::try_parse_from(["catalogue", "discover", "github"]).expect("parses");
        let Command::Discover {
            command: DiscoverCommand::Github { data, cache_dir },
        } = cli.command
        else {
            panic!("expected discover github");
        };
        assert_eq!((data, cache_dir), (PathBuf::from("data"), None));
        let cli = Cli::try_parse_from([
            "catalogue",
            "discover",
            "github",
            "--data",
            "d",
            "--cache-dir",
            "c",
        ])
        .expect("parses");
        let Command::Discover {
            command: DiscoverCommand::Github { data, cache_dir },
        } = cli.command
        else {
            panic!("expected discover github");
        };
        assert_eq!(
            (data, cache_dir),
            (PathBuf::from("d"), Some(PathBuf::from("c")))
        );
    }

    #[test]
    fn rustsec_fetch_and_show_take_an_optional_cache_dir() {
        let cli = Cli::try_parse_from(["catalogue", "rustsec", "fetch", "--cache-dir", "c"])
            .expect("parses");
        let Command::Rustsec {
            command: RustsecCommand::Fetch { cache_dir },
        } = cli.command
        else {
            panic!("expected rustsec fetch");
        };
        assert_eq!(cache_dir, Some(PathBuf::from("c")));
        let cli =
            Cli::try_parse_from(["catalogue", "rustsec", "show", "numr", "blazr"]).expect("parses");
        let Command::Rustsec {
            command: RustsecCommand::Show { names, cache_dir },
        } = cli.command
        else {
            panic!("expected rustsec show");
        };
        assert_eq!(
            (names, cache_dir),
            (vec!["numr".to_owned(), "blazr".to_owned()], None)
        );
        assert!(Cli::try_parse_from(["catalogue", "rustsec", "show"]).is_err());
    }

    #[test]
    fn a_subcommand_is_required() {
        assert!(Cli::try_parse_from(["catalogue"]).is_err());
    }
}
