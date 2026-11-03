//! `catalogue bot run`: the arguments and the exit code.

use std::path::PathBuf;
use std::process::ExitCode;

use catalogue_github::{ApiError, WRITE_INTERVAL};
use catalogue_signals::crates_io::CRATES_IO_INTERVAL;
use catalogue_signals::http::{IntervalPacer, ReqwestTransport};
use clap::Args;
use jiff::Timestamp;

use super::error::RunError;
use super::pass::run_pass;
use super::state::{STATE_DIRECTORY_VAR, state_dir};
use crate::bot::config::{CONFIG_VAR, config_path, load_config};
use crate::bot::labels::LabelsCommandError;
use crate::exit::IO;

/// The arguments of `catalogue bot run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// The bot config file. Default: $CATALOGUE_BOT_CONFIG, else bot.toml.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// The labels file. The run syncs the repo labels to it first.
    #[arg(long, value_name = "FILE", default_value = ".github/labels.toml")]
    pub labels: PathBuf,
}

/// Runs `catalogue bot run` and returns the process exit code.
///
/// Prints one line per checked issue on stdout, and warnings and failures on
/// stderr. The code is 0 when every issue is checked, 1 for a config, labels
/// file or key to fix, and 2 when an issue, a fetch, a PR step or the lock fails.
pub(crate) fn run_run(args: &RunArgs) -> ExitCode {
    let path = config_path(args.config.as_deref(), std::env::var_os(CONFIG_VAR));
    let result = load_config(&path)
        .map_err(|error| RunError::from(LabelsCommandError::from(error)))
        .and_then(|config| {
            let dir = state_dir(
                config.state_dir.as_deref(),
                std::env::var_os(STATE_DIRECTORY_VAR),
                std::env::var_os("XDG_CACHE_HOME"),
                std::env::var_os("HOME"),
            )?;
            let transport = ReqwestTransport::new().map_err(ApiError::from)?;
            let mut pacer = IntervalPacer::new(WRITE_INTERVAL);
            let mut crates_pacer = IntervalPacer::new(CRATES_IO_INTERVAL);
            let clock = Timestamp::now;
            run_pass(
                &transport,
                &mut pacer,
                &mut crates_pacer,
                &clock,
                &config,
                &args.labels,
                &dir,
            )
        });
    match result {
        Ok(report) => {
            for line in report.labels.plan.lines() {
                println!("{line}");
            }
            for line in &report.lines {
                println!("{line}");
            }
            for warning in &report.warnings {
                eprintln!("warning: {warning}");
            }
            for failure in &report.failures {
                eprintln!("error: {failure}");
            }
            if report.failures.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(IO)
            }
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
