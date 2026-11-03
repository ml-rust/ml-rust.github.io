//! `catalogue bot run`: the arguments and the exit code.

use std::path::PathBuf;
use std::process::ExitCode;

use catalogue_github::{ApiError, WRITE_INTERVAL};
use catalogue_signals::crates_io::CRATES_IO_INTERVAL;
use catalogue_signals::http::{IntervalPacer, ReqwestTransport, Transport};
use catalogue_triage::{MODEL_INTERVAL, SleepBackoff, WEB_FETCH_INTERVAL};
use clap::Args;
use jiff::Timestamp;

use super::error::RunError;
use super::pass::{PassDeps, run_pass};
use super::state::{STATE_DIRECTORY_VAR, state_dir};
use super::triage::TriageDeps;
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
/// stderr. With the link check on, the run reads each key variable the
/// config names from the environment. The code is 0 when every issue is
/// checked, 1 for a config, labels file or key to fix, and 2 when an issue, a
/// fetch, a PR step or the lock fails.
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
            let model_transport = if config.triage.enabled {
                Some(
                    ReqwestTransport::with_timeout(config.triage.request_timeout)
                        .map_err(ApiError::from)?,
                )
            } else {
                None
            };
            let model: &dyn Transport = match &model_transport {
                Some(found) => found,
                None => &transport,
            };
            let mut pacer = IntervalPacer::new(WRITE_INTERVAL);
            let mut crates_pacer = IntervalPacer::new(CRATES_IO_INTERVAL);
            let mut llm_pacer = IntervalPacer::new(MODEL_INTERVAL);
            let mut fetch_pacer = IntervalPacer::new(WEB_FETCH_INTERVAL);
            let mut backoff = SleepBackoff;
            let clock = Timestamp::now;
            let env = |name: &str| std::env::var(name).ok();
            let deps = PassDeps {
                transport: &transport,
                pacer: &mut pacer,
                crates_pacer: &mut crates_pacer,
                clock: &clock,
                triage: TriageDeps {
                    transport: model,
                    llm_pacer: &mut llm_pacer,
                    fetch_pacer: &mut fetch_pacer,
                    backoff: &mut backoff,
                    env: &env,
                },
            };
            run_pass(deps, &config, &args.labels, &dir)
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
