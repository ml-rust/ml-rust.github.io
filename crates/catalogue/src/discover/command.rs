//! The `catalogue discover` actions.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_core::Catalogue;
use catalogue_signals::discover::Rules;
use catalogue_signals::fetch;
use clap::Subcommand;

use super::below_floor::run_below_floor;
use super::error::DiscoverError;
use super::github::run_github;
use super::report::{Report, build_report};
use crate::cache_dir::resolve_cache_dir;
use crate::data_dir;
use crate::dump::{DumpCommandError, read_cached};
use crate::exit::INVALID;
use crate::seed::{DEFAULT_MIN_USERS, parse_min_users};

/// The `catalogue discover` actions.
#[derive(Debug, Subcommand)]
pub enum DiscoverCommand {
    /// List crates.io crates that match `discovery.toml` or the vocabulary of `topics.toml` and
    /// `categories.toml`, with their adoption signals.
    Crates {
        /// Directory that holds the data files.
        #[arg(long, value_name = "DIR", default_value = "data")]
        data: PathBuf,
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// Days of download history to keep, ending on the dump date.
        #[arg(long, value_name = "N", default_value_t = 90)]
        window_days: u16,
    },
    /// List GitHub repos that match `discovery.toml` and that no crates.io crate links to.
    ///
    /// Reads the token from the `GITHUB_TOKEN` environment variable and the
    /// cached crates.io dump. Run `catalogue dump fetch` first.
    Github {
        /// Directory that holds the data files.
        #[arg(long, value_name = "DIR", default_value = "data")]
        data: PathBuf,
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
    },
    /// List Rust repos of crates that match the vocabulary and have fewer active users than the floor.
    ///
    /// Finds projects that people run and no crate depends on, such as a database. This list is
    /// for review only: it writes no entry or lead, no tier reads it, and stars only sort it for a
    /// person. A reviewed repo enters through `data/leads.txt`. A repo left out goes in
    /// `data/leads-reviewed.txt`, and the list stops repeating it.
    ///
    /// Reads the token from the `GITHUB_TOKEN` environment variable and the cached crates.io
    /// dump. Run `catalogue dump fetch` first. The limits `min_stars` and `active_days` come from
    /// the `[below_floor]` table of `discovery.toml`.
    BelowFloor {
        /// Directory that holds the data files.
        #[arg(long, value_name = "DIR", default_value = "data")]
        data: PathBuf,
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// Days of download history to keep, ending on the dump date.
        #[arg(long, value_name = "N", default_value_t = 90)]
        window_days: u16,
        /// Fewest active dependents a crate has to leave this list. Default: 2.
        #[arg(
            long,
            value_name = "N",
            value_parser = parse_min_users,
            default_value_t = DEFAULT_MIN_USERS
        )]
        min_users: f64,
    },
}

/// Runs a discover action and returns the process exit code.
pub fn run_discover(command: DiscoverCommand) -> ExitCode {
    match command {
        DiscoverCommand::Crates {
            data,
            cache_dir,
            window_days,
        } => run_crates(&data, cache_dir, window_days),
        DiscoverCommand::Github { data, cache_dir } => run_github(&data, cache_dir),
        DiscoverCommand::BelowFloor {
            data,
            cache_dir,
            window_days,
            min_users,
        } => run_below_floor(&data, cache_dir, window_days, min_users),
    }
}

/// Prints the outcome of an action that fails and returns its exit code.
pub(crate) fn report_error(error: DiscoverError) -> ExitCode {
    if let DiscoverError::Invalid(diagnostics) = &error {
        for diagnostic in diagnostics {
            eprintln!("{diagnostic}");
        }
        return ExitCode::from(INVALID);
    }
    eprintln!("error: {error}");
    ExitCode::from(error.exit_code())
}

fn run_crates(data: &Path, cache_dir: Option<PathBuf>, window_days: u16) -> ExitCode {
    match discover_crates(data, cache_dir, window_days) {
        Ok(report) => {
            for name in &report.unknown_core_crates {
                eprintln!("warning: core crate {name} is not in the dump");
            }
            for name in &report.unknown_named_crates {
                eprintln!("warning: named crate {name} is not in the dump");
            }
            println!("{}", report.text);
            ExitCode::SUCCESS
        }
        Err(error) => report_error(error),
    }
}

/// Reads and checks the data directory, and returns the catalogue.
pub(crate) fn load_catalogue(data: &Path) -> Result<Catalogue, DiscoverError> {
    data_dir::load_catalogue(data)?.map_err(DiscoverError::Invalid)
}

/// The crates.io candidate rules of `catalogue`: `discovery.toml`, then the `keywords`,
/// `categories` and `terms` of `topics.toml` and `categories.toml`, sorted and without repeats.
///
/// `data` only names the rules file in the error.
pub(crate) fn crate_rules(catalogue: &Catalogue, data: &Path) -> Result<Rules, DiscoverError> {
    let Some(discovery) = catalogue.discovery() else {
        return Err(DiscoverError::NoRules {
            path: data.join("discovery.toml"),
        });
    };
    let mut categories = discovery.categories.clone();
    let mut keywords = discovery.keywords.clone();
    let mut terms = Vec::new();
    for topic in catalogue.topics() {
        categories.extend_from_slice(&topic.categories);
        keywords.extend_from_slice(&topic.keywords);
        terms.extend_from_slice(&topic.terms);
    }
    for category in catalogue.categories() {
        categories.extend_from_slice(&category.categories);
        keywords.extend_from_slice(&category.keywords);
        terms.extend_from_slice(&category.terms);
    }
    let own: HashSet<&String> = discovery
        .keywords
        .iter()
        .chain(&discovery.categories)
        .collect();
    let sort_only: HashSet<&String> = discovery.sort_only.iter().collect();
    for list in [&mut categories, &mut keywords, &mut terms] {
        list.retain(|word| own.contains(word) || !sort_only.contains(word));
        list.sort_unstable();
        list.dedup();
    }
    Ok(Rules {
        categories,
        keywords,
        core_crates: discovery
            .core_crates
            .iter()
            .map(ToString::to_string)
            .collect(),
        named_crates: discovery
            .named_crates
            .iter()
            .map(ToString::to_string)
            .collect(),
        terms,
    })
}

/// Checks the data, reads the cached dump and reports the candidates.
fn discover_crates(
    data: &Path,
    cache_dir: Option<PathBuf>,
    window_days: u16,
) -> Result<Report, DiscoverError> {
    let catalogue = load_catalogue(data)?;
    let rules = crate_rules(&catalogue, data)?;
    let mut listed: HashMap<String, &str> = HashMap::new();
    for (slug, entry) in catalogue.entries() {
        for name in &entry.crates {
            listed.insert(name.normalized(), slug.as_str());
        }
    }
    let cache_dir = resolve_cache_dir(cache_dir).map_err(DumpCommandError::from)?;
    let snapshot = read_cached(&fetch::dump_path(&cache_dir), window_days)?;
    Ok(build_report(&snapshot, &rules, &listed))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    fn manifest_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
    }

    #[test]
    fn the_rules_add_the_vocabulary_to_discovery_toml() {
        let data = manifest_path("tests/fixtures/valid");
        let catalogue = load_catalogue(&data).expect("valid");
        let rules = crate_rules(&catalogue, &data).expect("rules");
        assert!(rules.keywords.contains(&"llm".to_owned()));
        assert!(rules.keywords.contains(&"tensor".to_owned()));
        assert!(rules.categories.contains(&"science::ml".to_owned()));
        assert!(rules.terms.contains(&"ml framework".to_owned()));
        assert!(rules.terms.contains(&"tensor library".to_owned()));
        assert_eq!(rules.core_crates, ["burn", "candle-core"]);
        let mut sorted = rules.keywords.clone();
        sorted.dedup();
        assert_eq!(sorted, rules.keywords);
    }

    #[test]
    fn the_committed_rules_hold_the_terms_a_project_says_it_is() {
        let data = manifest_path("../../data");
        let catalogue = load_catalogue(&data).expect("valid");
        let rules = crate_rules(&catalogue, &data).expect("rules");
        for term in [
            "whisper cpp",
            "llama cpp",
            "hugging face",
            "huggingface",
            "model context protocol",
            "vector search engine",
            "xgboost",
            "lightgbm",
            "catboost",
            "langchain",
            "notebook environment",
        ] {
            assert!(rules.terms.contains(&term.to_owned()), "{term}");
        }
        assert!(rules.categories.contains(&"science::ml".to_owned()));
        assert!(!rules.categories.contains(&"science::math".to_owned()));
    }

    #[test]
    fn discover_crates_has_defaults_and_takes_options() {
        let cli = Cli::try_parse_from(["catalogue", "discover", "crates"]).expect("parses");
        let Command::Discover {
            command:
                DiscoverCommand::Crates {
                    data,
                    cache_dir,
                    window_days,
                },
        } = cli.command
        else {
            panic!("expected discover crates");
        };
        assert_eq!(
            (data, cache_dir, window_days),
            (PathBuf::from("data"), None, 90)
        );
        let cli = Cli::try_parse_from([
            "catalogue",
            "discover",
            "crates",
            "--data",
            "d",
            "--cache-dir",
            "c",
            "--window-days",
            "30",
        ])
        .expect("parses");
        let Command::Discover {
            command:
                DiscoverCommand::Crates {
                    data,
                    cache_dir,
                    window_days,
                },
        } = cli.command
        else {
            panic!("expected discover crates");
        };
        assert_eq!(
            (data, cache_dir, window_days),
            (PathBuf::from("d"), Some(PathBuf::from("c")), 30)
        );
    }

    #[test]
    fn a_sort_only_word_admits_no_crate_and_other_words_do() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let catalogue = load_catalogue(&data).expect("the data directory is valid");
        let rules = crate_rules(&catalogue, &data).expect("rules");
        let has = |list: &[String], word: &str| list.iter().any(|found| found == word);
        assert!(!has(&rules.terms, "pdf"));
        assert!(!has(&rules.keywords, "pdf"));
        assert!(!has(&rules.keywords, "numerics"));
        assert!(has(&rules.terms, "model context protocol"));
        assert!(has(&rules.terms, "whisper cpp"));
        assert!(has(&rules.keywords, "llm"));
    }
}
