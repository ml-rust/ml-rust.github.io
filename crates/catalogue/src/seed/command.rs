//! The `catalogue seed` command.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_core::{Catalogue, Categories, Diagnostics, Topics};
use catalogue_signals::discover::Rules;
use catalogue_signals::distribution::{DOCKER_INTERVAL, HOMEBREW_INTERVAL, REPOLOGY_INTERVAL};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::fetch;
use catalogue_signals::github::{GITHUB_INTERVAL, GithubError, RepoRef};
use catalogue_signals::http::{IntervalPacer, ReqwestTransport, Transport};
use catalogue_signals::seed::{Drafts, ExistingIndex, LeadInputs, LeadPacers, Mappings};
use clap::Args;

use super::build::build_seed;
use super::error::SeedError;
use super::leads::{LeadSeed, build_lead_seed, read_leads};
use super::merge::build_merged_seed;
use super::write::write_drafts;
use crate::cache_dir::resolve_cache_dir;
use crate::discover::{DiscoverError, crate_rules, load_catalogue};
use crate::dump::{DumpCommandError, read_cached};
use crate::token::token_from_env;

/// Days of download history read from the dump. The signals need none.
const WINDOW_DAYS: u16 = 1;

/// Fewest active dependents a crate has to draft when `--min-users` is unset.
pub(crate) const DEFAULT_MIN_USERS: f64 = 2.0;

/// The committed lead list, under the data directory.
pub(crate) const LEAD_FILE: &str = "leads.txt";

/// The arguments of `catalogue seed`.
///
/// Reads the cached crates.io dump. Run `catalogue dump fetch` first. When the data directory
/// holds `leads.txt`, the run also drafts from that list, which needs `GITHUB_TOKEN`.
#[derive(Debug, Args)]
pub struct SeedArgs {
    /// Directory that holds the data files.
    #[arg(long, value_name = "DIR", default_value = "data")]
    pub data: PathBuf,
    /// Directory that holds the cache. Default: the user cache directory.
    #[arg(long, value_name = "DIR")]
    pub cache_dir: Option<PathBuf>,
    /// Fewest active dependents a crate has to draft. Default: 2.
    ///
    /// A lead draft only gets the label `below-floor` below it.
    #[arg(long, value_name = "N", value_parser = parse_min_users)]
    pub min_users: Option<f64>,
    /// Write `entries/<slug>.toml` for each draft. Without it the run prints only.
    #[arg(long)]
    pub write: bool,
    /// Draft from this list of GitHub repos alone, one `owner/repo` per line.
    ///
    /// Reads the token from the `GITHUB_TOKEN` environment variable. The report of
    /// `catalogue discover github` also reads as a list. A lead bypasses the discovery rules and
    /// the active-user floor. A repo with a library crate makes a library draft. A repo with no
    /// crate or only binary crates makes an app draft.
    #[arg(long, value_name = "FILE")]
    pub leads: Option<PathBuf>,
    /// Leave `leads.txt` of the data directory out. The run then needs no token and no network.
    #[arg(long, conflicts_with = "leads")]
    pub no_leads: bool,
}

pub(crate) fn parse_min_users(text: &str) -> Result<f64, String> {
    match text.parse::<f64>() {
        Ok(value) if value.is_finite() && value >= 0.0 => Ok(value),
        _ => Err(format!("`{text}` is not a number of 0 or more")),
    }
}

/// Runs `catalogue seed` and returns the process exit code.
pub fn run_seed(args: SeedArgs) -> ExitCode {
    match seed(&args) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(SeedError::Discover(DiscoverError::Invalid(diagnostics))) => {
            print_diagnostics(&diagnostics);
            ExitCode::from(crate::exit::INVALID)
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn print_diagnostics(diagnostics: &Diagnostics) {
    for diagnostic in diagnostics {
        eprintln!("{diagnostic}");
    }
}

/// Checks the data, reads the cached dump, drafts the entries and returns the report.
///
/// With `--write` the report ends with `written: <n>` and the file paths.
fn seed(args: &SeedArgs) -> Result<String, SeedError> {
    let catalogue = load_catalogue(&args.data)?;
    if let Some(file) = &args.leads {
        return seed_from_leads(args, &catalogue, file);
    }
    let rules = crate_rules(&catalogue, &args.data)?;
    let lead_file = args.data.join(LEAD_FILE);
    if !args.no_leads && lead_file.exists() {
        return seed_merged(args, &catalogue, &rules, &lead_file);
    }
    let min_users = args.min_users.unwrap_or(DEFAULT_MIN_USERS);
    let snapshot = cached_dump(args)?;
    let (topics, categories) = mapping_files(&catalogue);
    let mappings = Mappings {
        topics: &topics,
        categories: &categories,
    };
    let existing = ExistingIndex::from_catalogue(&catalogue);
    let seeded = build_seed(&snapshot, &rules, &mappings, &existing, min_users);
    finish(args, seeded.text, &seeded.drafts)
}

/// Reads the lead file, resolves the leads, drafts the entries and returns the report.
///
/// The lead file and the token are checked before the dump is read. A failed channel lookup
/// prints a warning.
fn seed_from_leads(
    args: &SeedArgs,
    catalogue: &Catalogue,
    file: &Path,
) -> Result<String, SeedError> {
    run_leads(
        args,
        catalogue,
        file,
        |inputs, leads, transport, token, pacers| {
            build_lead_seed(inputs, leads, transport, token, pacers)
        },
    )
}

/// Drafts from the lead file and from discovery, and returns the merged report.
///
/// The lead run comes first, so a discovery draft never collides with a lead draft.
fn seed_merged(
    args: &SeedArgs,
    catalogue: &Catalogue,
    rules: &Rules,
    file: &Path,
) -> Result<String, SeedError> {
    run_leads(
        args,
        catalogue,
        file,
        |inputs, leads, transport, token, pacers| {
            build_merged_seed(inputs, rules, leads, transport, token, pacers)
        },
    )
}

/// Reads the lead file, the token and the dump, runs `build` and returns its report.
fn run_leads(
    args: &SeedArgs,
    catalogue: &Catalogue,
    file: &Path,
    build: impl FnOnce(
        &LeadInputs<'_>,
        &[RepoRef],
        &dyn Transport,
        &str,
        &mut LeadPacers<'_>,
    ) -> Result<LeadSeed, SeedError>,
) -> Result<String, SeedError> {
    let leads = read_leads(file)?;
    let token = token_from_env()?;
    let min_active_users = args.min_users.unwrap_or(DEFAULT_MIN_USERS);
    let snapshot = cached_dump(args)?;
    let (topics, categories) = mapping_files(catalogue);
    let existing = ExistingIndex::from_catalogue(catalogue);
    let inputs = LeadInputs {
        snapshot: &snapshot,
        mappings: Mappings {
            topics: &topics,
            categories: &categories,
        },
        existing: &existing,
        min_active_users,
    };
    let transport = ReqwestTransport::new().map_err(GithubError::from)?;
    let mut github = IntervalPacer::new(GITHUB_INTERVAL);
    let mut homebrew = IntervalPacer::new(HOMEBREW_INTERVAL);
    let mut docker = IntervalPacer::new(DOCKER_INTERVAL);
    let mut repology = IntervalPacer::new(REPOLOGY_INTERVAL);
    let mut pacers = LeadPacers {
        github: &mut github,
        homebrew: &mut homebrew,
        docker: &mut docker,
        repology: &mut repology,
    };
    let seeded = build(&inputs, &leads, &transport, &token, &mut pacers)?;
    for warning in &seeded.warnings {
        eprintln!("warning: {warning}");
    }
    finish(args, seeded.text, &seeded.drafts)
}

/// The topic and category tables of `catalogue`, in file order.
fn mapping_files(catalogue: &Catalogue) -> (Topics, Categories) {
    let topics = Topics {
        topic: catalogue.topics().to_vec(),
    };
    let categories = Categories {
        category: catalogue.categories().to_vec(),
    };
    (topics, categories)
}

/// Reads the cached crates.io dump.
fn cached_dump(args: &SeedArgs) -> Result<DumpSnapshot, SeedError> {
    let cache_dir = resolve_cache_dir(args.cache_dir.clone()).map_err(DumpCommandError::from)?;
    Ok(read_cached(&fetch::dump_path(&cache_dir), WINDOW_DAYS)?)
}

/// Returns the report. With `--write` it first writes the drafts and adds their paths.
fn finish(args: &SeedArgs, text: String, drafts: &Drafts) -> Result<String, SeedError> {
    if !args.write {
        return Ok(text);
    }
    let paths = write_drafts(&args.data, drafts)?;
    let mut lines = vec![text, format!("written: {}", paths.len())];
    lines.extend(paths.iter().map(|path| path.display().to_string()));
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    fn args(extra: &[&str]) -> Result<SeedArgs, clap::Error> {
        let mut argv = vec!["catalogue", "seed"];
        argv.extend_from_slice(extra);
        Cli::try_parse_from(argv).map(|cli| match cli.command {
            Command::Seed(args) => args,
            other => panic!("expected seed, got {other:?}"),
        })
    }

    #[test]
    fn defaults_print_only() {
        let args = args(&[]).expect("parses");
        assert_eq!(args.data, PathBuf::from("data"));
        assert_eq!(args.cache_dir, None);
        assert_eq!(args.min_users, None);
        assert!(!args.write);
        assert_eq!(args.leads, None);
        assert!(!args.no_leads);
    }

    #[test]
    fn no_leads_refuses_a_lead_file() {
        assert!(args(&["--no-leads"]).expect("parses").no_leads);
        assert!(args(&["--no-leads", "--leads", "leads.txt"]).is_err());
    }

    #[test]
    fn a_lead_file_is_read_beside_write() {
        let args = args(&["--leads", "leads.txt", "--write"]).expect("parses");
        assert_eq!(args.leads, Some(PathBuf::from("leads.txt")));
        assert!(args.write);
    }

    #[test]
    fn the_help_names_the_lead_file_and_the_token() {
        use clap::CommandFactory;

        let mut command = Cli::command();
        let seed = command.find_subcommand_mut("seed").expect("seed");
        let help = seed.render_long_help().to_string();
        assert!(help.contains("--leads <FILE>"), "{help}");
        assert!(help.contains("GITHUB_TOKEN"), "{help}");
    }

    #[test]
    fn options_are_read() {
        let args = args(&[
            "--data",
            "d",
            "--cache-dir",
            "c",
            "--min-users",
            "3.5",
            "--write",
        ])
        .expect("parses");
        assert_eq!(args.data, PathBuf::from("d"));
        assert_eq!(args.cache_dir, Some(PathBuf::from("c")));
        assert_eq!(args.min_users, Some(3.5));
        assert!(args.write);
    }

    #[test]
    fn min_users_must_be_a_non_negative_number() {
        for bad in ["-1", "nan", "inf", "x", ""] {
            assert!(args(&["--min-users", bad]).is_err(), "{bad}");
        }
        assert_eq!(
            args(&["--min-users", "0"]).expect("zero").min_users,
            Some(0.0)
        );
    }

    #[test]
    fn min_users_default_is_two() {
        assert_eq!(args(&[]).expect("parses").min_users, None);
        assert_eq!(DEFAULT_MIN_USERS, 2.0);
    }
}
