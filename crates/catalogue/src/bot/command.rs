//! The `catalogue bot` actions and their errors.

use std::fs;
use std::io;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::process::ExitCode;

use catalogue_bot::{CheckInput, CheckReport, FormKind, IssueMeta, check_submission, parse_form};
use catalogue_core::{Date, GithubLogin, IdError, Tier};
use clap::{Args, Subcommand};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use thiserror::Error;

use crate::data_dir::{DataDirError, read_data_dir};
use crate::exit::{INVALID, IO};

/// The `catalogue bot` actions.
#[derive(Debug, Subcommand)]
pub enum BotCommand {
    /// Check one issue form body and print the check report as markdown.
    ///
    /// Reads the body from a file and the data files from the data directory.
    /// Opens no connection. The issue number and author come from the caller,
    /// never from the body.
    Check(CheckArgs),
}

/// The arguments of `catalogue bot check`.
#[derive(Debug, Args)]
pub struct CheckArgs {
    /// The form label of the issue, such as `submit` or `judgment:kind`.
    #[arg(long, value_name = "LABEL", value_parser = parse_label)]
    pub label: FormKind,
    /// File that holds the issue body.
    #[arg(long, value_name = "FILE")]
    pub body: PathBuf,
    /// The issue number.
    #[arg(long, value_name = "N")]
    pub issue: NonZeroU64,
    /// The GitHub login of the issue author.
    #[arg(long, value_name = "LOGIN", value_parser = parse_login)]
    pub author: GithubLogin,
    /// The entry's current tier, for a demotion: featured, listed, new or archived.
    #[arg(long, value_name = "TIER", value_parser = parse_tier)]
    pub tier: Option<Tier>,
    /// Directory that holds the data files.
    #[arg(long, value_name = "DIR", default_value = "data")]
    pub data: PathBuf,
    /// The date that becomes `added` on a new entry, as YYYY-MM-DD. Default: today (UTC).
    #[arg(long, value_name = "DATE")]
    pub today: Option<jiff::civil::Date>,
}

/// A `catalogue bot` action that fails before it can report.
#[derive(Debug, Error)]
pub enum BotError {
    /// The body file cannot be read.
    #[error("cannot read issue body {}: {source}", .path.display())]
    Body {
        /// The body file.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// The data directory cannot be read.
    #[error(transparent)]
    Data(#[from] DataDirError),
}

fn parse_label(text: &str) -> Result<FormKind, String> {
    FormKind::from_label(text).ok_or_else(|| {
        format!(
            "`{text}` names no form. Use one of: {}",
            FormKind::label_list()
        )
    })
}

fn parse_login(text: &str) -> Result<GithubLogin, IdError> {
    GithubLogin::new(text)
}

fn parse_tier(text: &str) -> Result<Tier, String> {
    Tier::from_name(text).ok_or_else(|| {
        let names: Vec<&str> = Tier::ALL.into_iter().map(Tier::as_str).collect();
        format!("`{text}` is not a tier. Use one of: {}", names.join(", "))
    })
}

/// Reads the body and the data directory of `args` and checks the issue.
///
/// `today` becomes `added` on a new entry.
pub fn bot_check(args: &CheckArgs, today: Date) -> Result<CheckReport, BotError> {
    let body = fs::read_to_string(&args.body).map_err(|source| BotError::Body {
        path: args.body.clone(),
        source,
    })?;
    let data = read_data_dir(&args.data)?;
    let form = parse_form(args.label, &body);
    let meta = IssueMeta {
        number: args.issue,
        author: args.author.clone(),
    };
    Ok(check_submission(&CheckInput {
        main: &data.sources,
        kind: args.label,
        form: &form,
        meta: &meta,
        today,
        current_tier: args.tier,
    }))
}

/// Runs a bot action and returns the process exit code.
///
/// Prints the report on stdout. The code is 0 for a report with no blocking
/// error, 1 for a blocking report and 2 for a read error.
pub fn run_bot(command: BotCommand) -> ExitCode {
    match command {
        BotCommand::Check(args) => {
            let today = args
                .today
                .unwrap_or_else(|| Timestamp::now().to_zoned(TimeZone::UTC).date());
            match bot_check(&args, Date::from(today)) {
                Ok(report) => {
                    print!("{}", report.markdown());
                    if report.blocking {
                        ExitCode::from(INVALID)
                    } else {
                        ExitCode::SUCCESS
                    }
                }
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::from(IO)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    fn parse(extra: &[&str]) -> Result<CheckArgs, clap::Error> {
        let mut argv = vec![
            "catalogue",
            "bot",
            "check",
            "--label",
            "judgment:demotion",
            "--body",
            "body.md",
            "--issue",
            "12",
            "--author",
            "dave",
        ];
        argv.extend_from_slice(extra);
        let cli = Cli::try_parse_from(argv)?;
        let Command::Bot {
            command: BotCommand::Check(args),
        } = cli.command
        else {
            panic!("expected bot check");
        };
        Ok(args)
    }

    #[test]
    fn check_takes_the_issue_facts() {
        let args = parse(&["--tier", "featured", "--today", "2026-11-03"]).expect("parses");
        assert_eq!(args.label, FormKind::Demotion);
        assert_eq!(args.body, PathBuf::from("body.md"));
        assert_eq!(args.issue.get(), 12);
        assert_eq!(args.author.as_str(), "dave");
        assert_eq!(args.tier, Some(Tier::Featured));
        assert_eq!(args.data, PathBuf::from("data"));
        assert_eq!(args.today, Some(jiff::civil::date(2026, 11, 3)));
    }

    /// Parses `bot check` with one argument replaced: `flag` takes `value`.
    fn parse_with(flag: &str, value: &str) -> Result<Cli, clap::Error> {
        let mut argv = vec![
            ("--label", "submit"),
            ("--body", "b"),
            ("--issue", "1"),
            ("--author", "dave"),
        ];
        for pair in &mut argv {
            if pair.0 == flag {
                pair.1 = value;
            }
        }
        let mut words = vec!["catalogue", "bot", "check"];
        for (name, text) in argv {
            words.push(name);
            words.push(text);
        }
        Cli::try_parse_from(words)
    }

    #[test]
    fn check_rejects_bad_facts() {
        assert!(parse_with("--label", "submit").is_ok());
        assert!(parse_with("--label", "Submit").is_err());
        assert!(parse_with("--label", "judgment:").is_err());
        assert!(parse_with("--issue", "0").is_err());
        assert!(parse_with("--issue", "-3").is_err());
        assert!(parse_with("--author", "-x").is_err());
        assert!(parse(&["--tier", "gold"]).is_err());
    }

    #[test]
    fn a_missing_body_is_a_read_error() {
        let args = parse(&[]).expect("parses");
        let args = CheckArgs {
            body: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/no-such-body.md"),
            ..args
        };
        let today = Date::new(2026, 11, 3).expect("date");
        assert!(matches!(
            bot_check(&args, today),
            Err(BotError::Body { .. })
        ));
    }
}
