//! The `catalogue rustsec` actions and the cached archive reader.

use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_signals::fetch;
use catalogue_signals::rustsec::{self, Advisories, UNPATCHED_GRACE_DAYS};
use clap::Subcommand;
use jiff::civil;

use super::error::RustsecCommandError;
use super::render::render_crate;
use crate::cache_dir::resolve_cache_dir;
use crate::clock::today_utc;
use crate::dump::render_outcome;

/// Bytes buffered when reading the archive file.
const READ_BUFFER: usize = 1 << 20;

/// The `catalogue rustsec` actions.
#[derive(Debug, Subcommand)]
pub enum RustsecCommand {
    /// Download the RustSec advisory database, or revalidate the cached copy.
    Fetch {
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
    },
    /// Read the cached archive and print the advisories of each named crate.
    Show {
        /// The crate names to look up.
        #[arg(required = true, value_name = "CRATE")]
        names: Vec<String>,
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
    },
}

/// Runs a rustsec action and returns the process exit code.
pub fn run_rustsec(command: RustsecCommand) -> ExitCode {
    match run(command) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn run(command: RustsecCommand) -> Result<String, RustsecCommandError> {
    match command {
        RustsecCommand::Fetch { cache_dir } => {
            let cache_dir = resolve_cache_dir(cache_dir)?;
            Ok(render_outcome(&fetch::rustsec(&cache_dir)?))
        }
        RustsecCommand::Show { names, cache_dir } => {
            let cache_dir = resolve_cache_dir(cache_dir)?;
            let advisories = read_cached(&fetch::rustsec_path(&cache_dir))?;
            eprintln!("warnings: {}", advisories.warnings.len());
            Ok(show(&advisories, &names, today_utc()))
        }
    }
}

/// The blocks of `rustsec show` for `names`, separated by a blank line.
fn show(advisories: &Advisories, names: &[String], today: civil::Date) -> String {
    names
        .iter()
        .map(|name| render_crate(advisories, name, today, UNPATCHED_GRACE_DAYS))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Reads the archive file at `path`.
pub(crate) fn read_cached(path: &Path) -> Result<Advisories, RustsecCommandError> {
    let file = File::open(path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            RustsecCommandError::Missing {
                path: path.to_path_buf(),
            }
        } else {
            RustsecCommandError::Open {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    rustsec::read(BufReader::with_capacity(READ_BUFFER, file)).map_err(|source| {
        RustsecCommandError::Read {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use jiff::civil::date;

    use super::*;
    use crate::cli::{Cli, Command};
    use crate::exit::IO;

    #[test]
    fn missing_archive_names_the_path_and_the_fetch_command() {
        let error = read_cached(Path::new("/nonexistent/advisory-db.tar.gz")).expect_err("missing");
        let message = error.to_string();
        assert!(
            message.contains("/nonexistent/advisory-db.tar.gz"),
            "{message}"
        );
        assert!(message.contains("catalogue rustsec fetch"), "{message}");
        assert_eq!(error.exit_code(), IO);
    }

    #[test]
    fn show_separates_crates_with_a_blank_line() {
        let names = ["a".to_owned(), "b".to_owned()];
        let text = show(&Advisories::default(), &names, date(2026, 11, 3));
        assert_eq!(text.matches("\n\n").count(), 1);
        assert!(text.starts_with("a\n") && text.contains("\n\nb\n"));
    }

    #[test]
    fn rustsec_fetch_and_show_take_an_optional_cache_dir() {
        let cli = Cli::try_parse_from(["catalogue", "rustsec", "fetch", "--cache-dir", "c"])
            .expect("parses");
        let Command::Rustsec {
            command: RustsecCommand::Fetch { cache_dir },
        } = cli.command
        else {
            panic!("expected rustsec fetch");
        };
        assert_eq!(cache_dir, Some(PathBuf::from("c")));
        let cli =
            Cli::try_parse_from(["catalogue", "rustsec", "show", "numr", "blazr"]).expect("parses");
        let Command::Rustsec {
            command: RustsecCommand::Show { names, cache_dir },
        } = cli.command
        else {
            panic!("expected rustsec show");
        };
        assert_eq!(
            (names, cache_dir),
            (vec!["numr".to_owned(), "blazr".to_owned()], None)
        );
        assert!(Cli::try_parse_from(["catalogue", "rustsec", "show"]).is_err());
    }
}
