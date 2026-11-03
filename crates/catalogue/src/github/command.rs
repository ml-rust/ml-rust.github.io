//! The `catalogue github` actions and their errors.

use std::process::ExitCode;

use catalogue_signals::github::{
    GithubError, Pacer, RepoFacts, RepoRef, RepoRefError, SleepPacer, repo_facts,
};
use catalogue_signals::http::{Headers, ReqwestTransport};
use clap::Subcommand;
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use thiserror::Error;

use super::render::render_all;
use crate::exit::{INVALID, IO};
use crate::token::{NoToken, token_from_env};

/// The `catalogue github` actions.
#[derive(Debug, Subcommand)]
pub enum GithubCommand {
    /// Print the facts of GitHub repos: state, human commits and maintainer response times.
    ///
    /// Reads the token from the `GITHUB_TOKEN` environment variable. Prints one
    /// `key: value` line per fact and a blank line between repos.
    Facts {
        /// The repos to read, each as `owner/name`.
        #[arg(required = true, value_name = "OWNER/REPO")]
        repos: Vec<String>,
        /// The day the 365-day windows end on, as YYYY-MM-DD. Default: today (UTC).
        #[arg(long, value_name = "DATE")]
        today: Option<Date>,
    },
}

/// A `catalogue github` action that fails.
#[derive(Debug, Error)]
pub enum GithubCommandError {
    /// An argument is not `owner/name`.
    #[error(transparent)]
    Repo(#[from] RepoRefError),
    /// `GITHUB_TOKEN` is unset or empty.
    #[error(transparent)]
    Token(#[from] NoToken),
    /// A GitHub request fails.
    #[error(transparent)]
    Github(#[from] GithubError),
}

impl GithubCommandError {
    /// The process exit code.
    ///
    /// 1 for a bad repo argument, a missing token and a rejected token. 2 for
    /// IO, network and API errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Repo(_)
            | Self::Token(_)
            | Self::Github(GithubError::Status { status: 401, .. }) => INVALID,
            Self::Github(_) => IO,
        }
    }
}

/// Runs a github action and returns the process exit code.
pub fn run_github(command: GithubCommand) -> ExitCode {
    let GithubCommand::Facts { repos, today } = command;
    match facts(&repos, today, token_from_env()) {
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

/// Parses every argument as `owner/name`.
fn parse_repos(arguments: &[String]) -> Result<Vec<RepoRef>, RepoRefError> {
    arguments.iter().map(|text| RepoRef::parse(text)).collect()
}

/// Reads the facts of each repo and renders them.
fn facts(
    arguments: &[String],
    today: Option<Date>,
    token: Result<String, NoToken>,
) -> Result<String, GithubCommandError> {
    let repos = parse_repos(arguments)?;
    let token = token?;
    let today = today.unwrap_or_else(|| Timestamp::now().to_zoned(TimeZone::UTC).date());
    let transport = ReqwestTransport::new().map_err(GithubError::from)?;
    let mut pacer = SleepPacer;
    let mut results: Vec<(RepoRef, RepoFacts)> = Vec::with_capacity(repos.len());
    for (index, repo) in repos.into_iter().enumerate() {
        if index > 0 {
            pacer.wait(&Headers::default());
        }
        let found = repo_facts(&transport, &token, &repo, today, &mut pacer)?;
        results.push((repo, found));
    }
    Ok(render_all(&results))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    #[test]
    fn facts_takes_repos_and_a_date() {
        let cli = Cli::try_parse_from([
            "catalogue",
            "github",
            "facts",
            "a/b",
            "c/d",
            "--today",
            "2026-11-03",
        ])
        .expect("parses");
        let Command::Github {
            command: GithubCommand::Facts { repos, today },
        } = cli.command
        else {
            panic!("expected github facts");
        };
        assert_eq!(repos, ["a/b", "c/d"]);
        assert_eq!(today, Some(jiff::civil::date(2026, 11, 3)));
    }

    #[test]
    fn facts_needs_a_repo_and_a_real_date() {
        assert!(Cli::try_parse_from(["catalogue", "github", "facts"]).is_err());
        assert!(
            Cli::try_parse_from(["catalogue", "github", "facts", "a/b", "--today", "soon"])
                .is_err()
        );
    }

    #[test]
    fn a_bad_argument_is_named_and_exits_one() {
        let error = facts(
            &["a/b".to_owned(), "nope".to_owned()],
            None,
            Ok("t".to_owned()),
        )
        .expect_err("bad repo");
        assert!(error.to_string().contains("`nope`"), "{error}");
        assert_eq!(error.exit_code(), INVALID);
    }

    #[test]
    fn a_bad_argument_is_reported_before_the_token() {
        let error = facts(&["nope".to_owned()], None, Err(NoToken)).expect_err("bad repo");
        assert!(matches!(error, GithubCommandError::Repo(_)));
    }

    #[test]
    fn a_missing_token_exits_one() {
        let error = facts(&["a/b".to_owned()], None, Err(NoToken)).expect_err("no token");
        assert!(error.to_string().contains("set GITHUB_TOKEN"), "{error}");
        assert_eq!(error.exit_code(), INVALID);
    }

    #[test]
    fn api_errors_carry_their_exit_codes() {
        let status = |status| {
            GithubCommandError::Github(GithubError::Status {
                url: "u".to_owned(),
                status,
                action: "x",
            })
        };
        assert_eq!(status(401).exit_code(), INVALID);
        assert_eq!(status(500).exit_code(), IO);
    }
}
