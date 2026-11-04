//! Runs `catalogue build`: arguments, the build steps and the exit code.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_core::{Catalogue, Date, Movement, Slug, TierReport};
use catalogue_signals::history::{History, read_all};
use catalogue_site::{SiteInputs, TierChangeRow, build_site};
use clap::{ArgGroup, Args};

use super::error::BuildError;
use super::publish::replace_dir;
use crate::data_dir::load_catalogue;
use crate::exit::INVALID;

/// The arguments of `catalogue build`.
///
/// The build writes the pages, the stylesheet, the search page, its script, the
/// search index, the Atom feed, the data file, `llms.txt`, `sitemap.xml` and
/// `robots.txt`. It does not write `search/oxidoc_search.js` or
/// `search/oxidoc_search_bg.wasm`: the deploy adds both with
/// `scripts/ci/build_search_wasm.sh`.
#[derive(Debug, Args)]
#[command(group(ArgGroup::new("tiers").required(true).args(["report", "no_tiers"])))]
pub struct BuildArgs {
    /// Directory that holds the data files.
    #[arg(long, value_name = "DIR", default_value = "data")]
    pub data: PathBuf,
    /// Tier report from `catalogue tiers --json`. Pages show the tiers in it.
    /// The build refuses uncalibrated thresholds and a stale report.
    #[arg(long, value_name = "FILE")]
    pub report: Option<PathBuf>,
    /// Build every page without a tier.
    #[arg(long)]
    pub no_tiers: bool,
    /// Directory of daily snapshots from `catalogue tiers --history`. Feeds the
    /// changes page, the feed and the movement words.
    #[arg(
        long,
        value_name = "DIR",
        requires = "report",
        conflicts_with = "no_tiers"
    )]
    pub history: Option<PathBuf>,
    /// Output directory. The build replaces it whole and removes files that a
    /// previous build wrote. It must not hold the data, report or history.
    #[arg(long, value_name = "DIR", default_value = "dist")]
    pub out: PathBuf,
}

/// What one build wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSummary {
    /// Number of files written.
    pub files: usize,
    /// The output directory.
    pub out: PathBuf,
}

impl std::fmt::Display for BuildSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ok: {} files in {}", self.files, self.out.display())
    }
}

/// Runs `catalogue build` and returns the process exit code.
pub fn run_build(args: &BuildArgs) -> ExitCode {
    match build(args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(BuildError::Invalid(diagnostics)) => {
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

/// Checks the data, reads the report and history, builds the site and
/// replaces the output directory with it.
///
/// # Errors
///
/// Returns a [`BuildError`]; its `exit_code` gives the process exit code.
pub fn build(args: &BuildArgs) -> Result<BuildSummary, BuildError> {
    let catalogue = load_catalogue(&args.data)?.map_err(BuildError::Invalid)?;
    let report = match &args.report {
        Some(path) => Some(read_report(path)?),
        None => None,
    };
    let history = match &args.history {
        Some(dir) => read_history(dir, &catalogue)?,
        None => HistoryFacts::default(),
    };
    let files = build_site(&SiteInputs {
        catalogue: &catalogue,
        report: report.as_ref(),
        changes: &history.changes,
        first_check: history.first_check,
        movement: &history.movement,
    })?;
    let mut protected: Vec<&Path> = vec![&args.data];
    protected.extend(args.report.as_deref());
    protected.extend(args.history.as_deref());
    let current = std::env::current_dir().ok();
    protected.extend(current.as_deref());
    replace_dir(&args.out, &files, &protected)?;
    Ok(BuildSummary {
        files: files.len(),
        out: args.out.clone(),
    })
}

fn read_report(path: &Path) -> Result<TierReport, BuildError> {
    let text = fs::read_to_string(path).map_err(|source| BuildError::ReportRead {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|error| BuildError::ReportParse {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

/// What the pages read from a history.
#[derive(Debug, Default)]
struct HistoryFacts {
    changes: Vec<TierChangeRow>,
    first_check: Option<Date>,
    movement: Movement,
}

/// The tier changes, the first check and the movement of the history at `dir`.
fn read_history(dir: &Path, catalogue: &Catalogue) -> Result<HistoryFacts, BuildError> {
    let history = read_all(dir)?;
    let movement = catalogue
        .thresholds()
        .map_or(Movement::NeedsHistory, |thresholds| {
            history.movement(&thresholds.movement)
        });
    Ok(HistoryFacts {
        changes: change_rows(&history)?,
        first_check: history.first_check(),
        movement,
    })
}

/// Every tier change in `history`, newest first.
///
/// A change for a slug that the catalogue lacks stays: the changes page shows
/// it as plain text.
fn change_rows(history: &History) -> Result<Vec<TierChangeRow>, BuildError> {
    history
        .tier_changes()
        .into_iter()
        .map(|change| {
            let slug = Slug::new(change.slug.clone())
                .map_err(|_| BuildError::HistorySlug { slug: change.slug })?;
            Ok(TierChangeRow {
                date: change.date,
                slug,
                from: change.from,
                to: change.to,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    fn parse(arguments: &[&str]) -> Result<BuildArgs, clap::Error> {
        let mut full = vec!["catalogue", "build"];
        full.extend_from_slice(arguments);
        Cli::try_parse_from(full).map(|cli| match cli.command {
            Command::Build(args) => args,
            other => panic!("expected build, got {other:?}"),
        })
    }

    #[test]
    fn defaults_apply() {
        let args = parse(&["--no-tiers"]).expect("parses");
        assert_eq!(args.data, PathBuf::from("data"));
        assert_eq!(args.out, PathBuf::from("dist"));
        assert!(args.no_tiers);
        assert_eq!(args.report, None);
        assert_eq!(args.history, None);
    }

    #[test]
    fn exactly_one_of_report_and_no_tiers_is_required() {
        assert!(parse(&[]).is_err());
        assert!(parse(&["--report", "r.json", "--no-tiers"]).is_err());
        assert!(parse(&["--report", "r.json"]).is_ok());
    }

    #[test]
    fn history_needs_a_report() {
        assert!(parse(&["--no-tiers", "--history", "h"]).is_err());
        assert!(parse(&["--report", "r.json", "--history", "h"]).is_ok());
    }

    #[test]
    fn a_missing_report_names_the_file() {
        let error = read_report(Path::new("/nonexistent/r.json")).expect_err("missing");
        assert!(error.to_string().contains("/nonexistent/r.json"), "{error}");
        assert_eq!(error.exit_code(), crate::exit::IO);
    }
}
