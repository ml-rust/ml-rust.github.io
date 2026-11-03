use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::validate::{Outcome, validate};

/// Exit code when the data has errors.
const EXIT_INVALID: u8 = 1;
/// Exit code when the data directory cannot be read.
const EXIT_IO: u8 = 2;

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
}

/// Runs the parsed command and returns the process exit code.
///
/// The code is 0 for valid data, 1 for data errors and 2 for a read error.
pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Command::Validate { data } => run_validate(&data),
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
            ExitCode::from(EXIT_INVALID)
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(EXIT_IO)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_defaults_to_the_data_directory() {
        let cli = Cli::try_parse_from(["catalogue", "validate"]).expect("parses");
        let Command::Validate { data } = cli.command;
        assert_eq!(data, PathBuf::from("data"));
    }

    #[test]
    fn validate_takes_a_data_directory() {
        let cli =
            Cli::try_parse_from(["catalogue", "validate", "--data", "other"]).expect("parses");
        let Command::Validate { data } = cli.command;
        assert_eq!(data, PathBuf::from("other"));
    }

    #[test]
    fn a_subcommand_is_required() {
        assert!(Cli::try_parse_from(["catalogue"]).is_err());
    }
}
