//! The `catalogue sandbox` actions.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use catalogue_core::Date;
use catalogue_signals::fetch;
use catalogue_signals::history::read_all;
use clap::{Args, Subcommand};

use super::check::{CheckRun, check};
use super::error::SandboxCommandError;
use super::files::write_items;
use super::list::sandbox_list;
use crate::cache_dir::resolve_cache_dir;
use crate::clock::today_utc;
use crate::data_dir::load_catalogue;
use crate::dump::read_cached;
use crate::exit::INVALID;

/// Days of download history read from the dump. The list reads no downloads.
const DOWNLOAD_WINDOW_DAYS: u16 = 0;

/// The `catalogue sandbox` actions.
#[derive(Debug, Subcommand)]
pub enum SandboxCommand {
    /// Write the crates to check: every crate of each dormant library entry.
    List(ListArgs),
    /// Run `cargo check` on each listed crate release and write the results.
    ///
    /// Third-party build scripts run, so the command refuses unless
    /// `GITHUB_ACTIONS` is `true` and `RUNNER_TEMP` is set. Run it only in an
    /// ephemeral CI job with no secrets.
    Check(CheckArgs),
}

/// The arguments of `catalogue sandbox list`.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Directory that holds the data files.
    #[arg(long, value_name = "DIR", default_value = "data")]
    pub data: PathBuf,
    /// Directory that holds the cache. Default: the user cache directory.
    #[arg(long, value_name = "DIR")]
    pub cache_dir: Option<PathBuf>,
    /// Directory of daily snapshots from `catalogue tiers --history`. The newest
    /// snapshot decides which entries are dormant. A missing directory lists no crate.
    #[arg(long, value_name = "DIR")]
    pub history: PathBuf,
    /// The list file to write, one JSON line per crate release.
    #[arg(long, value_name = "FILE")]
    pub out: PathBuf,
}

/// The arguments of `catalogue sandbox check`.
#[derive(Debug, Args)]
pub struct CheckArgs {
    /// The list file of `catalogue sandbox list`.
    #[arg(long, value_name = "FILE")]
    pub list: PathBuf,
    /// The result file to write, one JSON line per crate release.
    #[arg(long, value_name = "FILE")]
    pub out: PathBuf,
    /// The toolchain label each result records. Default: the output of `rustc --version`.
    #[arg(long, value_name = "TEXT")]
    pub toolchain_label: Option<String>,
    /// The time limit of each crate's build, in minutes.
    #[arg(long, value_name = "N", default_value_t = 15)]
    pub timeout_minutes: u16,
}

/// Runs a sandbox action and returns the process exit code.
pub fn run_sandbox(command: SandboxCommand) -> ExitCode {
    let result = match command {
        SandboxCommand::List(args) => list(&args),
        SandboxCommand::Check(args) => run_check(&args),
    };
    match result {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(SandboxCommandError::Invalid(diagnostics)) => {
            for diagnostic in &diagnostics {
                eprintln!("{diagnostic}");
            }
            ExitCode::from(INVALID)
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

/// Writes the list file and returns the `crates: <n>` summary with one line per note.
fn list(args: &ListArgs) -> Result<String, SandboxCommandError> {
    let catalogue = load_catalogue(&args.data)?.map_err(SandboxCommandError::Invalid)?;
    let history = read_all(&args.history)?;
    let cache_dir = resolve_cache_dir(args.cache_dir.clone())?;
    let dump = read_cached(&fetch::dump_path(&cache_dir), DOWNLOAD_WINDOW_DAYS)?;
    let today = Date::from(today_utc());
    let found = sandbox_list(&catalogue, &history, &dump, today);
    write_items(&args.out, &found.items)?;
    let mut lines = vec![format!("crates: {}", found.items.len())];
    lines.extend(found.notes);
    Ok(lines.join("\n"))
}

/// The value of variable `name` in the process environment.
fn process_env(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

/// Runs the checks and returns the result counts.
fn run_check(args: &CheckArgs) -> Result<String, SandboxCommandError> {
    let run = CheckRun {
        list: &args.list,
        out: &args.out,
        toolchain_label: args.toolchain_label.as_deref(),
        timeout_minutes: args.timeout_minutes,
    };
    let counts = check(&run, process_env)?;
    Ok(format!(
        "checked: {}\npasses: {}\nfails: {}\ninconclusive: {}",
        counts
            .passes
            .saturating_add(counts.fails)
            .saturating_add(counts.inconclusive),
        counts.passes,
        counts.fails,
        counts.inconclusive
    ))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    fn parse(arguments: &[&str]) -> SandboxCommand {
        let mut full = vec!["catalogue", "sandbox"];
        full.extend_from_slice(arguments);
        let Command::Sandbox { command } = Cli::try_parse_from(full).expect("parses").command
        else {
            panic!("expected sandbox");
        };
        command
    }

    #[test]
    fn list_needs_a_history_and_an_out_file() {
        let SandboxCommand::List(args) = parse(&["list", "--history", "h", "--out", "o"]) else {
            panic!("expected list");
        };
        assert_eq!(args.data, PathBuf::from("data"));
        assert_eq!(args.history, PathBuf::from("h"));
        assert_eq!(args.out, PathBuf::from("o"));
        assert!(Cli::try_parse_from(["catalogue", "sandbox", "list", "--out", "o"]).is_err());
    }

    #[test]
    fn check_defaults_to_a_15_minute_limit() {
        let SandboxCommand::Check(args) = parse(&["check", "--list", "l", "--out", "o"]) else {
            panic!("expected check");
        };
        assert_eq!(args.timeout_minutes, 15);
        assert_eq!(args.toolchain_label, None);
        let SandboxCommand::Check(args) = parse(&[
            "check",
            "--list",
            "l",
            "--out",
            "o",
            "--toolchain-label",
            "rustc 1.95.0",
            "--timeout-minutes",
            "5",
        ]) else {
            panic!("expected check");
        };
        assert_eq!(args.timeout_minutes, 5);
        assert_eq!(args.toolchain_label.as_deref(), Some("rustc 1.95.0"));
    }
}
