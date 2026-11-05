//! The `catalogue tiers` command.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_core::{Catalogue, Date, Deprecated, Thresholds};
use catalogue_signals::fallback::FetchSummary;
use catalogue_signals::fetch;
use catalogue_signals::github::GITHUB_INTERVAL;
use catalogue_signals::http::{IntervalPacer, ReqwestTransport};
use catalogue_signals::sandbox::BuildChecks;
use clap::Args;

use super::collect::Network;
use super::error::TiersError;
use super::inputs::{Inputs, Outputs, TiersRun};
use super::run::run_with;
use super::step_summary::{STEP_SUMMARY_VAR, append_step_summary};
use crate::cache_dir::resolve_cache_dir;
use crate::clock::today_utc;
use crate::data_dir::load_catalogue;
use crate::dump::read_cached_with_details;
use crate::exit::INVALID;
use crate::rustsec::read_cached as read_advisories;
use crate::sandbox::read_build_checks;
use crate::token::{NoToken, token_from_env};

/// Days of download history read from the dump: the `downloads_per_month` window.
const DOWNLOAD_WINDOW_DAYS: u16 = 30;

/// The arguments of `catalogue tiers`.
#[derive(Debug, Args)]
pub struct TiersArgs {
    /// Directory that holds the data files.
    #[arg(long, value_name = "DIR", default_value = "data")]
    pub data: PathBuf,
    /// Directory that holds the cache. Default: the user cache directory.
    #[arg(long, value_name = "DIR")]
    pub cache_dir: Option<PathBuf>,
    /// Send no request. Every repo and distribution signal reads as unknown.
    #[arg(long)]
    pub offline: bool,
    /// Directory of daily snapshots. The run reads it for the previous tier, the
    /// dead run, the Docker baseline and the values a failed fetch falls back to,
    /// then adds today's snapshot. Without it the run reads and writes no history,
    /// so a failed fetch leaves its signal unknown. An offline run holds only
    /// unknown network signals, so `--offline` conflicts with it.
    #[arg(long, value_name = "DIR", conflicts_with = "offline")]
    pub history: Option<PathBuf>,
    /// Result file of `catalogue sandbox check`, for rot signal R6. A result
    /// counts only for a crate's latest release. Without it R6 reads as unknown.
    #[arg(long, value_name = "FILE")]
    pub build_checks: Option<PathBuf>,
    /// Also write the tier report as JSON to FILE: each entry's tier, reasons,
    /// notes and every signal with its source link and data date. The file is
    /// replaced atomically after the history write.
    #[arg(long, value_name = "FILE")]
    pub json: Option<PathBuf>,
}

/// Runs `catalogue tiers` and returns the process exit code.
pub fn run_tiers(args: TiersArgs) -> ExitCode {
    let today = Date::from(today_utc());
    match tiers(&args, token_from_env, today) {
        Ok(run) => {
            println!("{}", run.text);
            write_step_summary(&run.summary);
            ExitCode::SUCCESS
        }
        Err(TiersError::Invalid(diagnostics)) => {
            for diagnostic in &diagnostics {
                eprintln!("{diagnostic}");
            }
            ExitCode::from(INVALID)
        }
        Err(error) => {
            eprintln!("error: {error}");
            if let TiersError::TooManyFailed { summary, .. } = &error {
                write_step_summary(summary);
            }
            ExitCode::from(error.exit_code())
        }
    }
}

/// Appends `summary` to the job summary file when the environment names one.
///
/// A write error is a warning. The run itself already holds true dates.
fn write_step_summary(summary: &FetchSummary) {
    if summary.fetches == 0 {
        return;
    }
    let Some(path) = std::env::var_os(STEP_SUMMARY_VAR) else {
        return;
    };
    let path = Path::new(&path);
    if let Err(error) = append_step_summary(path, summary) {
        eprintln!(
            "warning: cannot append the fallback list to {}: {error}. The run goes on",
            path.display()
        );
    }
}

/// Checks the data, reads the caches, fetches the network signals and renders the report.
///
/// The token is read only for an online run, after the data checks and before the dump read.
/// With `--json`, the tier report goes to that file after the history write.
fn tiers(
    args: &TiersArgs,
    token: impl FnOnce() -> Result<String, NoToken>,
    today: Date,
) -> Result<TiersRun, TiersError> {
    let catalogue = load(&args.data)?;
    required(&catalogue)?;
    let token = if args.offline { None } else { Some(token()?) };
    let cache_dir = resolve_cache_dir(args.cache_dir.clone())?;
    let listed: Vec<&str> = catalogue
        .entries()
        .values()
        .flat_map(|entry| &entry.crates)
        .map(|name| name.as_str())
        .collect();
    let snapshot =
        read_cached_with_details(&fetch::dump_path(&cache_dir), DOWNLOAD_WINDOW_DAYS, &listed)?;
    let advisories = read_advisories(&fetch::rustsec_path(&cache_dir))?;
    let build_checks = match &args.build_checks {
        Some(path) => read_build_checks(path)?,
        None => BuildChecks::default(),
    };
    let inputs = Inputs {
        catalogue: &catalogue,
        snapshot: &snapshot,
        advisories: &advisories,
        build_checks: &build_checks,
        today,
    };
    let outputs = Outputs {
        history_dir: args.history.as_deref(),
        json: args.json.as_deref(),
    };
    let mut github = IntervalPacer::new(GITHUB_INTERVAL);
    match token {
        None => run_with(&inputs, Network::Offline, &outputs, &mut github),
        Some(token) => {
            let transport = ReqwestTransport::new()?;
            let network = Network::Online {
                transport: &transport,
                token: &token,
                cache_dir: &cache_dir,
            };
            run_with(&inputs, network, &outputs, &mut github)
        }
    }
}

/// The thresholds and the deprecated list, which the tier rules need.
pub(super) fn required(catalogue: &Catalogue) -> Result<(&Thresholds, &Deprecated), TiersError> {
    let thresholds = catalogue.thresholds().ok_or(TiersError::MissingFile {
        name: "thresholds.toml",
    })?;
    let deprecated = catalogue.deprecated().ok_or(TiersError::MissingFile {
        name: "deprecated.toml",
    })?;
    Ok((thresholds, deprecated))
}

fn load(data: &Path) -> Result<Catalogue, TiersError> {
    load_catalogue(data)?.map_err(TiersError::Invalid)
}

#[cfg(test)]
mod tests {
    use catalogue_core::{DataSources, SourceFile};
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};
    use crate::exit::IO;

    fn parse(arguments: &[&str]) -> TiersArgs {
        let mut full = vec!["catalogue", "tiers"];
        full.extend_from_slice(arguments);
        let Command::Tiers(args) = Cli::try_parse_from(full).expect("parses").command else {
            panic!("expected tiers");
        };
        args
    }

    fn fixture(case: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(case)
    }

    fn today() -> Date {
        Date::new(2026, 11, 3).expect("date")
    }

    fn args(data: PathBuf, offline: bool) -> TiersArgs {
        TiersArgs {
            data,
            cache_dir: Some(PathBuf::from("/nonexistent/cache")),
            offline,
            history: None,
            build_checks: None,
            json: None,
        }
    }

    #[test]
    fn defaults_and_options_parse() {
        let args = parse(&[]);
        assert_eq!(args.data, PathBuf::from("data"));
        assert_eq!(args.cache_dir, None);
        assert!(!args.offline);
        assert_eq!(args.history, None);
        assert_eq!(args.build_checks, None);
        assert_eq!(args.json, None);
        let args = parse(&["--data", "d", "--cache-dir", "c", "--offline"]);
        assert_eq!(args.data, PathBuf::from("d"));
        assert_eq!(args.cache_dir, Some(PathBuf::from("c")));
        assert!(args.offline);
        let args = parse(&["--history", "h", "--build-checks", "b.jsonl"]);
        assert_eq!(args.history, Some(PathBuf::from("h")));
        assert_eq!(args.build_checks, Some(PathBuf::from("b.jsonl")));
        let args = parse(&["--offline", "--json", "report.json"]);
        assert!(args.offline);
        assert_eq!(args.json, Some(PathBuf::from("report.json")));
    }

    #[test]
    fn history_conflicts_with_offline() {
        let parsed = Cli::try_parse_from(["catalogue", "tiers", "--offline", "--history", "h"]);
        assert!(parsed.is_err());
    }

    #[test]
    fn invalid_data_exits_one_before_the_token_or_the_caches() {
        let invalid = args(fixture("duplicate-crate"), false);
        let error = tiers(&invalid, || Err(NoToken), today()).expect_err("invalid");
        assert!(matches!(error, TiersError::Invalid(_)));
        assert_eq!(error.exit_code(), INVALID);
    }

    #[test]
    fn thresholds_and_the_deprecated_list_are_required() {
        let thresholds = SourceFile {
            path: "thresholds.toml".to_owned(),
            contents: include_str!("../../../../data/thresholds.toml").to_owned(),
        };
        let empty = Catalogue::load(DataSources::default()).expect("empty data");
        let error = required(&empty).expect_err("no thresholds");
        assert_eq!(
            error.to_string(),
            "no thresholds.toml in the data directory. Create the file, then rerun"
        );
        assert_eq!(error.exit_code(), INVALID);
        let sources = DataSources {
            thresholds: Some(thresholds),
            ..DataSources::default()
        };
        let partial = Catalogue::load(sources).expect("thresholds only");
        let error = required(&partial).expect_err("no deprecated list");
        assert!(error.to_string().contains("no deprecated.toml"), "{error}");
    }

    #[test]
    fn a_missing_data_directory_exits_two() {
        let missing = fixture("valid").join("absent");
        let error = tiers(&args(missing, true), || Err(NoToken), today()).expect_err("missing");
        assert!(matches!(error, TiersError::Data(_)));
        assert_eq!(error.exit_code(), IO);
    }

    #[test]
    fn a_missing_token_exits_one() {
        let online = args(fixture("valid"), false);
        let error = tiers(&online, || Err(NoToken), today()).expect_err("no token");
        assert!(matches!(error, TiersError::Token(_)));
        assert_eq!(error.exit_code(), INVALID);
    }

    #[test]
    fn a_missing_dump_names_the_fetch_command_and_exits_two() {
        let offline = args(fixture("valid"), true);
        let error = tiers(&offline, || Err(NoToken), today()).expect_err("no dump");
        let message = error.to_string();
        assert!(message.contains("catalogue dump fetch"), "{message}");
        assert_eq!(error.exit_code(), IO);
    }
}
