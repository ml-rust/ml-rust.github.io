//! The `catalogue tiers` command.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_core::{Catalogue, Date, Deprecated, Thresholds, TierReport};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::fallback::FetchSummary;
use catalogue_signals::fetch;
use catalogue_signals::history::{History, Snapshot, read_all, write};
use catalogue_signals::http::ReqwestTransport;
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::BuildChecks;
use clap::Args;

use super::assess::assess_entries;
use super::collect::{Network, fetch_all};
use super::error::TiersError;
use super::render::render_tiers;
use super::report::{tier_report, write_report};
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

/// The outcome of one `catalogue tiers` check.
#[derive(Debug, Clone, PartialEq)]
pub struct TiersRun {
    /// The text report.
    pub text: String,
    /// The tier report, with the source and data date of each signal.
    pub report: TierReport,
    /// The network fetches of the run and the ones that failed. Empty offline.
    pub summary: FetchSummary,
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
    let run = match token {
        None => tiers_report(
            &catalogue,
            &snapshot,
            &advisories,
            &build_checks,
            Network::Offline,
            args.history.as_deref(),
            today,
        )?,
        Some(token) => {
            let transport = ReqwestTransport::new()?;
            let network = Network::Online {
                transport: &transport,
                token: &token,
                cache_dir: &cache_dir,
            };
            tiers_report(
                &catalogue,
                &snapshot,
                &advisories,
                &build_checks,
                network,
                args.history.as_deref(),
                today,
            )?
        }
    };
    if let Some(path) = &args.json {
        write_report(path, &run.report)?;
    }
    Ok(run)
}

/// The `catalogue tiers` report for a checked catalogue on `today`.
///
/// Fetches the network signals through `network`, assembles each entry's
/// signals, applies the tier rules and renders one block per entry, in slug order.
/// The same pass builds the [`TierReport`], with the source and data date of each signal.
/// `build_checks` holds the sandbox results that rot signal R6 reads.
///
/// With `history_dir`, the run reads the snapshots in that directory before it
/// applies the rules, then writes the snapshot of `today` to it. Without it,
/// the run reads and writes no history.
///
/// A network fetch that fails falls back to the previous snapshot, and an online
/// run ends its text with the `fallbacks:` block. The exit stays successful when
/// most fetches fail: the block warns, and every value keeps the date of its data.
///
/// # Errors
///
/// Returns [`TiersError::MissingFile`] when the catalogue lacks
/// `thresholds.toml` or `deprecated.toml`, the fatal request error that ends an
/// online run, such as a rejected token, and [`TiersError::History`] when the
/// history directory cannot be read or written.
pub fn tiers_report(
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    advisories: &Advisories,
    build_checks: &BuildChecks,
    network: Network<'_>,
    history_dir: Option<&Path>,
    today: Date,
) -> Result<TiersRun, TiersError> {
    let (thresholds, _) = required(catalogue)?;
    let history = match history_dir {
        Some(dir) => read_all(dir)?,
        None => History::default(),
    };
    let online = matches!(network, Network::Online { .. });
    let fetched = fetch_all(
        catalogue,
        snapshot,
        network,
        &history,
        thresholds.fallback.max_age_days,
        today,
    )?;
    let assessments = assess_entries(
        catalogue,
        snapshot,
        advisories,
        build_checks,
        &fetched.entries,
        &history,
        today,
    )?;
    if let Some(dir) = history_dir {
        let rows = assessments
            .iter()
            .map(|assessment| (assessment.slug.to_string(), assessment.row.clone()))
            .collect();
        write(dir, &Snapshot { date: today, rows })?;
    }
    let mut text = render_tiers(&assessments);
    if online {
        text.push_str("\n\n");
        text.push_str(&fetched.summary.text());
    }
    Ok(TiersRun {
        text,
        report: tier_report(&assessments, thresholds, today),
        summary: fetched.summary,
    })
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
