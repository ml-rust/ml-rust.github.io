//! The `catalogue tiers` command and its errors.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_core::{Catalogue, Date, Deprecated, Diagnostics, Thresholds};
use catalogue_signals::distribution::DistributionError;
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::fetch;
use catalogue_signals::github::GithubError;
use catalogue_signals::http::{HttpError, ReqwestTransport};
use catalogue_signals::link::LinkError;
use catalogue_signals::rustsec::Advisories;
use clap::Args;
use jiff::Timestamp;
use jiff::tz::TimeZone;
use thiserror::Error;

use super::assess::assess_entries;
use super::collect::{Network, fetch_all};
use super::render::render_tiers;
use crate::cache_dir::{CacheDirError, resolve_cache_dir};
use crate::data_dir::{DataDirError, load_catalogue};
use crate::dump::{DumpCommandError, read_cached};
use crate::exit::{INVALID, IO};
use crate::rustsec::{RustsecCommandError, read_cached as read_advisories};
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
}

/// A `catalogue tiers` run that fails.
#[derive(Debug, Error)]
pub enum TiersError {
    /// The data directory cannot be read.
    #[error(transparent)]
    Data(#[from] DataDirError),
    /// The data files break at least one rule.
    #[error(transparent)]
    Invalid(Diagnostics),
    /// A data file the tier rules need is absent.
    #[error("no {name} in the data directory. Create the file, then rerun")]
    MissingFile {
        /// The file name.
        name: &'static str,
    },
    /// The cache directory cannot be derived.
    #[error(transparent)]
    CacheDir(#[from] CacheDirError),
    /// The cached dump cannot be found or read.
    #[error(transparent)]
    Dump(#[from] DumpCommandError),
    /// The cached advisory archive cannot be found or read.
    #[error(transparent)]
    Rustsec(#[from] RustsecCommandError),
    /// `GITHUB_TOKEN` is unset or empty.
    #[error(transparent)]
    Token(#[from] NoToken),
    /// The HTTP client cannot start.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// A GitHub facts request fails.
    #[error(transparent)]
    Github(#[from] GithubError),
    /// A release, Homebrew or Repology request fails.
    #[error(transparent)]
    Distribution(#[from] DistributionError),
    /// A repo link check fails.
    #[error(transparent)]
    Link(#[from] LinkError),
}

impl TiersError {
    /// The process exit code.
    ///
    /// 1 for data, a missing data file, a missing token and a rejected token. 2
    /// for IO and network errors. The dump and archive codes for their errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Invalid(_)
            | Self::MissingFile { .. }
            | Self::Token(_)
            | Self::Github(GithubError::Status { status: 401, .. })
            | Self::Link(LinkError::Github(GithubError::Status { status: 401, .. }))
            | Self::Distribution(DistributionError::Status { status: 401, .. }) => INVALID,
            Self::Dump(error) => error.exit_code(),
            Self::Rustsec(error) => error.exit_code(),
            Self::Data(_)
            | Self::CacheDir(_)
            | Self::Http(_)
            | Self::Github(_)
            | Self::Link(_)
            | Self::Distribution(_) => IO,
        }
    }
}

/// Runs `catalogue tiers` and returns the process exit code.
pub fn run_tiers(args: TiersArgs) -> ExitCode {
    let today = Date::from(Timestamp::now().to_zoned(TimeZone::UTC).date());
    match tiers(&args, token_from_env, today) {
        Ok(text) => {
            println!("{text}");
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

/// Checks the data, reads the caches, fetches the network signals and renders the report.
///
/// The token is read only for an online run, after the data checks and before the dump read.
fn tiers(
    args: &TiersArgs,
    token: impl FnOnce() -> Result<String, NoToken>,
    today: Date,
) -> Result<String, TiersError> {
    let catalogue = load(&args.data)?;
    required(&catalogue)?;
    let token = if args.offline { None } else { Some(token()?) };
    let cache_dir = resolve_cache_dir(args.cache_dir.clone())?;
    let snapshot = read_cached(&fetch::dump_path(&cache_dir), DOWNLOAD_WINDOW_DAYS)?;
    let advisories = read_advisories(&fetch::rustsec_path(&cache_dir))?;
    match token {
        None => tiers_report(&catalogue, &snapshot, &advisories, Network::Offline, today),
        Some(token) => {
            let transport = ReqwestTransport::new()?;
            let network = Network::Online {
                transport: &transport,
                token: &token,
                cache_dir: &cache_dir,
            };
            tiers_report(&catalogue, &snapshot, &advisories, network, today)
        }
    }
}

/// The `catalogue tiers` report for a checked catalogue on `today`.
///
/// Fetches the network signals through `network`, assembles each entry's
/// signals, applies the tier rules and renders one block per entry, in slug order.
///
/// # Errors
///
/// Returns [`TiersError::MissingFile`] when the catalogue lacks
/// `thresholds.toml` or `deprecated.toml`, and the request error that ends an
/// online run.
pub fn tiers_report(
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    advisories: &Advisories,
    network: Network<'_>,
    today: Date,
) -> Result<String, TiersError> {
    let fetched = fetch_all(catalogue, snapshot, network, today.civil())?;
    let assessments = assess_entries(catalogue, snapshot, advisories, &fetched, today)?;
    Ok(render_tiers(&assessments))
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
        }
    }

    #[test]
    fn defaults_and_options_parse() {
        let args = parse(&[]);
        assert_eq!(args.data, PathBuf::from("data"));
        assert_eq!(args.cache_dir, None);
        assert!(!args.offline);
        let args = parse(&["--data", "d", "--cache-dir", "c", "--offline"]);
        assert_eq!(args.data, PathBuf::from("d"));
        assert_eq!(args.cache_dir, Some(PathBuf::from("c")));
        assert!(args.offline);
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
