//! The `catalogue ecosystems` actions and their errors.

use std::process::ExitCode;

use catalogue_signals::ecosystems::{
    CrateDependents, ECOSYSTEMS_INTERVAL, EcosystemsError, crate_dependents,
};
use catalogue_signals::http::{Headers, IntervalPacer, Pacer, ReqwestTransport};
use clap::Subcommand;
use thiserror::Error;

use super::render::render_all;
use crate::exit::{INVALID, IO};

/// The `catalogue ecosystems` actions.
#[derive(Debug, Subcommand)]
pub enum EcosystemsCommand {
    /// Print how many packages and repos depend on crates.io crates.
    ///
    /// Prints one `key: value` line per count and a blank line between crates.
    /// A crate that ecosyste.ms does not know prints `-`.
    Crates {
        /// The crates to look up.
        #[arg(required = true, value_name = "NAME")]
        names: Vec<String>,
    },
}

/// A `catalogue ecosystems` action that fails.
#[derive(Debug, Error)]
pub enum EcosystemsCommandError {
    /// A crate name is empty.
    #[error("a crate name is empty. Pass the name of a crates.io crate")]
    EmptyName,
    /// An ecosyste.ms request fails.
    #[error(transparent)]
    Ecosystems(#[from] EcosystemsError),
}

impl EcosystemsCommandError {
    /// The process exit code.
    ///
    /// 1 for an empty crate name. 2 for IO, network and API errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::EmptyName => INVALID,
            Self::Ecosystems(_) => IO,
        }
    }
}

/// Runs an ecosystems action and returns the process exit code.
pub fn run_ecosystems(command: EcosystemsCommand) -> ExitCode {
    let EcosystemsCommand::Crates { names } = command;
    match crates(&names) {
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

/// Looks up each crate and renders the counts.
fn crates(names: &[String]) -> Result<String, EcosystemsCommandError> {
    if names.iter().any(|name| name.trim().is_empty()) {
        return Err(EcosystemsCommandError::EmptyName);
    }
    let transport = ReqwestTransport::new().map_err(EcosystemsError::from)?;
    let mut pacer = IntervalPacer::new(ECOSYSTEMS_INTERVAL);
    let mut results: Vec<(String, Option<CrateDependents>)> = Vec::with_capacity(names.len());
    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            pacer.wait(&Headers::default());
        }
        let found = crate_dependents(&transport, name, &mut pacer)?;
        results.push((name.clone(), found));
    }
    Ok(render_all(&results))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    #[test]
    fn crates_takes_names() {
        let cli = Cli::try_parse_from(["catalogue", "ecosystems", "crates", "numr", "blazr"])
            .expect("parses");
        let Command::Ecosystems {
            command: EcosystemsCommand::Crates { names },
        } = cli.command
        else {
            panic!("expected ecosystems crates");
        };
        assert_eq!(names, ["numr", "blazr"]);
    }

    #[test]
    fn crates_needs_a_name() {
        assert!(Cli::try_parse_from(["catalogue", "ecosystems", "crates"]).is_err());
    }

    #[test]
    fn an_empty_name_exits_one_before_any_request() {
        let error = crates(&["numr".to_owned(), " ".to_owned()]).expect_err("empty name");
        assert!(matches!(error, EcosystemsCommandError::EmptyName));
        assert_eq!(error.exit_code(), INVALID);
    }

    #[test]
    fn api_errors_exit_two() {
        let error = EcosystemsCommandError::Ecosystems(EcosystemsError::RetriesExhausted {
            url: "u".to_owned(),
            retries: 3,
        });
        assert_eq!(error.exit_code(), IO);
    }
}
