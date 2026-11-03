//! The `catalogue github` actions and their errors.

use std::path::PathBuf;
use std::process::ExitCode;

use catalogue_signals::github::{
    GITHUB_INTERVAL, GithubError, RepoFacts, RepoRef, RepoRefError, repo_facts,
};
use catalogue_signals::http::{IntervalPacer, ReqwestTransport};
use catalogue_signals::link::LinkError;
use clap::Subcommand;
use jiff::civil::Date;
use thiserror::Error;

use super::link::link;
use super::render::render_all;
use crate::batch;
use crate::cache_dir::CacheDirError;
use crate::clock::today_utc;
use crate::dump::DumpCommandError;
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
    /// Check that a crate was published from a commit on the default branch of its repo.
    ///
    /// Reads the token from the `GITHUB_TOKEN` environment variable and the
    /// latest unyanked version from the cached crates.io dump. Downloads the
    /// package, reads its `.cargo_vcs_info.json` and compares the commit with
    /// the default branch on GitHub. A commit that only a fork holds does not
    /// verify. Prints the keys `crate`, `version`, `sha1`, `result` and
    /// `cached`. `result` is `verified`, `commit-missing`,
    /// `not-on-default-branch`, `no-vcs-info` or `unsupported-host`.
    Link {
        /// The repo, as `owner/name`.
        #[arg(value_name = "OWNER/REPO")]
        repo: String,
        /// The crate to check.
        #[arg(value_name = "CRATE")]
        crate_name: String,
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
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
    /// The cache directory cannot be derived.
    #[error(transparent)]
    CacheDir(#[from] CacheDirError),
    /// The cached dump cannot be read, or lacks the crate.
    #[error(transparent)]
    Dump(#[from] DumpCommandError),
    /// The crate has no unyanked version in the dump.
    #[error("crate {name} has no unyanked version in the dump. Check the crate name, then rerun")]
    NoRelease {
        /// The crate name.
        name: String,
    },
    /// The link check fails.
    #[error(transparent)]
    Link(#[from] LinkError),
}

impl GithubCommandError {
    /// The process exit code.
    ///
    /// 1 for a bad repo argument, a missing token, a rejected token and a crate
    /// without a release. 2 for IO, network and API errors. The dump codes for
    /// dump errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Repo(_)
            | Self::Token(_)
            | Self::NoRelease { .. }
            | Self::Github(GithubError::Status { status: 401, .. })
            | Self::Link(LinkError::Github(GithubError::Status { status: 401, .. })) => INVALID,
            Self::Dump(error) => error.exit_code(),
            Self::Github(_) | Self::CacheDir(_) | Self::Link(_) => IO,
        }
    }
}

/// Runs a github action and returns the process exit code.
pub fn run_github(command: GithubCommand) -> ExitCode {
    let result = match command {
        GithubCommand::Facts { repos, today } => facts(&repos, today, token_from_env()),
        GithubCommand::Link {
            repo,
            crate_name,
            cache_dir,
        } => link(&repo, &crate_name, cache_dir, token_from_env()),
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
    let today = today.unwrap_or_else(today_utc);
    let transport = ReqwestTransport::new().map_err(GithubError::from)?;
    let mut pacer = IntervalPacer::new(GITHUB_INTERVAL);
    let found: Vec<RepoFacts> = batch::each(&repos, &mut pacer, |repo, pacer| {
        repo_facts(&transport, &token, repo, today, pacer)
    })?;
    let results: Vec<(RepoRef, RepoFacts)> = repos.into_iter().zip(found).collect();
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
    fn link_takes_a_repo_a_crate_and_a_cache_directory() {
        let cli =
            Cli::try_parse_from(["catalogue", "github", "link", "a/b", "krate"]).expect("parses");
        let Command::Github {
            command:
                GithubCommand::Link {
                    repo,
                    crate_name,
                    cache_dir,
                },
        } = cli.command
        else {
            panic!("expected github link");
        };
        assert_eq!((repo.as_str(), crate_name.as_str()), ("a/b", "krate"));
        assert_eq!(cache_dir, None);
        let cli = Cli::try_parse_from([
            "catalogue",
            "github",
            "link",
            "a/b",
            "krate",
            "--cache-dir",
            "c",
        ])
        .expect("parses");
        let Command::Github {
            command: GithubCommand::Link { cache_dir, .. },
        } = cli.command
        else {
            panic!("expected github link");
        };
        assert_eq!(cache_dir, Some(PathBuf::from("c")));
        assert!(Cli::try_parse_from(["catalogue", "github", "link", "a/b"]).is_err());
    }

    #[test]
    fn link_reports_a_bad_repo_before_the_token_and_a_missing_token_exits_one() {
        let error = link("nope", "k", None, Err(NoToken)).expect_err("bad repo");
        assert!(matches!(error, GithubCommandError::Repo(_)));
        let error = link("a/b", "k", None, Err(NoToken)).expect_err("no token");
        assert!(matches!(error, GithubCommandError::Token(_)));
        assert_eq!(error.exit_code(), INVALID);
    }

    #[test]
    fn link_exit_codes_follow_the_cause() {
        let github = |status| {
            GithubCommandError::Link(LinkError::Github(GithubError::Status {
                url: "u".to_owned(),
                status,
                action: "x",
            }))
        };
        assert_eq!(github(401).exit_code(), INVALID);
        assert_eq!(github(500).exit_code(), IO);
        let none = GithubCommandError::NoRelease {
            name: "k".to_owned(),
        };
        assert_eq!(none.exit_code(), INVALID);
        let unknown = GithubCommandError::Dump(DumpCommandError::UnknownCrates {
            names: vec!["k".to_owned()],
        });
        assert_eq!(unknown.exit_code(), INVALID);
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
