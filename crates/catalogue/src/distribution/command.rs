//! The `catalogue distribution` actions and their errors.

use std::process::ExitCode;

use catalogue_signals::distribution::DistributionError;
use catalogue_signals::github::RepoRefError;
use clap::Subcommand;
use thiserror::Error;

use super::lookup::{docker, homebrew, releases, repology};
use crate::exit::{INVALID, IO};
use crate::token::{NoToken, token_from_env};

/// The `catalogue distribution` actions.
///
/// Each action prints one `key: value` line per count and a blank line between
/// names. A name that the API does not know prints `-`.
#[derive(Debug, Subcommand)]
pub enum DistributionCommand {
    /// Print the binary downloads of the latest GitHub releases of repos.
    ///
    /// Sums the last 3 non-draft releases. Checksum and signature files do not
    /// count. Reads the token from the `GITHUB_TOKEN` environment variable.
    Releases {
        /// The repos to read, each as `owner/name`.
        #[arg(required = true, value_name = "OWNER/REPO")]
        repos: Vec<String>,
    },
    /// Print the Homebrew install counts of formulae.
    Homebrew {
        /// The formulae to read.
        #[arg(required = true, value_name = "FORMULA")]
        formulas: Vec<String>,
    },
    /// Print the Docker Hub pull counts of repos.
    ///
    /// One run gives the current count. Growth needs two runs.
    Docker {
        /// The repos to read, each as `namespace/repo`.
        #[arg(required = true, value_name = "NAMESPACE/REPO")]
        repos: Vec<String>,
    },
    /// Print which distribution repos package projects, per Repology.
    Repology {
        /// The Repology projects to read.
        #[arg(required = true, value_name = "PROJECT")]
        projects: Vec<String>,
    },
}

/// A `catalogue distribution` action that fails.
#[derive(Debug, Error)]
pub enum DistributionCommandError {
    /// An argument is not `owner/name`.
    #[error(transparent)]
    Repo(#[from] RepoRefError),
    /// An argument is not `namespace/repo`.
    #[error(
        "`{input}` is not a Docker Hub repo in the form namespace/repo. Pass it as namespace/repo, for example library/nginx"
    )]
    DockerName {
        /// The rejected argument.
        input: String,
    },
    /// A name is empty.
    #[error("a name is empty. Pass the name of a formula or project")]
    EmptyName,
    /// `GITHUB_TOKEN` is unset or empty.
    #[error(transparent)]
    Token(#[from] NoToken),
    /// A distribution request fails.
    #[error(transparent)]
    Distribution(#[from] DistributionError),
}

impl DistributionCommandError {
    /// The process exit code.
    ///
    /// 1 for a bad argument, a missing token and a rejected token. 2 for IO,
    /// network and API errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Repo(_)
            | Self::DockerName { .. }
            | Self::EmptyName
            | Self::Token(_)
            | Self::Distribution(DistributionError::Status { status: 401, .. }) => INVALID,
            Self::Distribution(_) => IO,
        }
    }
}

/// Runs a distribution action and returns the process exit code.
pub fn run_distribution(command: DistributionCommand) -> ExitCode {
    match run(&command, token_from_env()) {
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

/// Runs an action and renders its report.
fn run(
    command: &DistributionCommand,
    token: Result<String, NoToken>,
) -> Result<String, DistributionCommandError> {
    match command {
        DistributionCommand::Releases { repos } => releases(repos, token),
        DistributionCommand::Homebrew { formulas } => homebrew(formulas),
        DistributionCommand::Docker { repos } => docker(repos),
        DistributionCommand::Repology { projects } => repology(projects),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    fn parse(arguments: &[&str]) -> DistributionCommand {
        let mut full = vec!["catalogue", "distribution"];
        full.extend_from_slice(arguments);
        let Command::Distribution { command } = Cli::try_parse_from(full).expect("parses").command
        else {
            panic!("expected distribution");
        };
        command
    }

    #[test]
    fn every_action_takes_names() {
        assert!(matches!(
            parse(&["releases", "a/b", "c/d"]),
            DistributionCommand::Releases { repos } if repos == ["a/b", "c/d"]
        ));
        assert!(matches!(
            parse(&["homebrew", "ripgrep"]),
            DistributionCommand::Homebrew { formulas } if formulas == ["ripgrep"]
        ));
        assert!(matches!(
            parse(&["docker", "library/nginx"]),
            DistributionCommand::Docker { repos } if repos == ["library/nginx"]
        ));
        assert!(matches!(
            parse(&["repology", "ripgrep"]),
            DistributionCommand::Repology { projects } if projects == ["ripgrep"]
        ));
    }

    #[test]
    fn every_action_needs_a_name() {
        for action in ["releases", "homebrew", "docker", "repology"] {
            assert!(Cli::try_parse_from(["catalogue", "distribution", action]).is_err());
        }
        assert!(Cli::try_parse_from(["catalogue", "distribution"]).is_err());
    }

    #[test]
    fn a_rejected_token_exits_one_and_other_api_errors_exit_two() {
        let unauthorized = DistributionCommandError::Distribution(DistributionError::Status {
            api: "GitHub Releases",
            url: "u".to_owned(),
            status: 401,
            action: "set GITHUB_TOKEN to a valid token",
        });
        assert_eq!(unauthorized.exit_code(), INVALID);
        let exhausted =
            DistributionCommandError::Distribution(DistributionError::RetriesExhausted {
                api: "Repology",
                url: "u".to_owned(),
                retries: 3,
            });
        assert_eq!(exhausted.exit_code(), IO);
    }
}
