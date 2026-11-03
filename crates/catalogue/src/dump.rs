use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_signals::dump::{self, DumpError, DumpSnapshot};
use catalogue_signals::fetch::{self, FetchError, FetchOutcome};
use clap::Subcommand;
use thiserror::Error;

use crate::cache_dir::{CacheDirError, resolve_cache_dir};

/// Exit code for a dump that does not parse.
const EXIT_DATA: u8 = 1;
/// Exit code for a read, write or network error.
const EXIT_IO: u8 = 2;
/// Bytes buffered when reading the dump file.
const READ_BUFFER: usize = 1 << 20;

/// The `catalogue dump` actions.
#[derive(Debug, Subcommand)]
pub enum DumpCommand {
    /// Download the crates.io database dump, or resume an interrupted download.
    Fetch {
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
    },
    /// Read the cached dump and print what it holds.
    Stats {
        /// Directory that holds the cache. Default: the user cache directory.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// Days of download history to keep, ending on the dump date.
        #[arg(long, value_name = "N", default_value_t = 90)]
        window_days: u16,
    },
}

/// A `catalogue dump` action that fails.
#[derive(Debug, Error)]
pub enum DumpCommandError {
    /// The cache directory cannot be derived.
    #[error(transparent)]
    CacheDir(#[from] CacheDirError),
    /// The download fails.
    #[error(transparent)]
    Fetch(#[from] FetchError),
    /// The cached dump file is absent.
    #[error("no dump at {}. Run `catalogue dump fetch` first", .path.display())]
    Missing {
        /// The expected dump file.
        path: PathBuf,
    },
    /// The cached dump file cannot be opened.
    #[error("{}: {source}. Check the path and its permissions, then rerun", .path.display())]
    Open {
        /// The dump file.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// The dump content is rejected.
    #[error("{}: {source}", .path.display())]
    Read {
        /// The dump file.
        path: PathBuf,
        /// The reader error.
        source: DumpError,
    },
}

impl DumpCommandError {
    /// The process exit code: 1 for dump content, 2 for IO and network.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Read { source, .. } if !matches!(source, DumpError::Io { .. }) => EXIT_DATA,
            _ => EXIT_IO,
        }
    }
}

/// Runs a dump action and returns the process exit code.
pub fn run_dump(command: DumpCommand) -> ExitCode {
    let result = match command {
        DumpCommand::Fetch { cache_dir } => fetch_dump(cache_dir),
        DumpCommand::Stats {
            cache_dir,
            window_days,
        } => stats_dump(cache_dir, window_days),
    };
    match result {
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

fn fetch_dump(cache_dir: Option<PathBuf>) -> Result<String, DumpCommandError> {
    let cache_dir = resolve_cache_dir(cache_dir)?;
    Ok(render_outcome(&fetch::dump(&cache_dir)?))
}

fn stats_dump(cache_dir: Option<PathBuf>, window_days: u16) -> Result<String, DumpCommandError> {
    let cache_dir = resolve_cache_dir(cache_dir)?;
    let snapshot = read_cached(&fetch::dump_path(&cache_dir), window_days)?;
    Ok(render_stats(&snapshot, window_days))
}

fn read_cached(path: &Path, window_days: u16) -> Result<DumpSnapshot, DumpCommandError> {
    let file = File::open(path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            DumpCommandError::Missing {
                path: path.to_path_buf(),
            }
        } else {
            DumpCommandError::Open {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    dump::read(BufReader::with_capacity(READ_BUFFER, file), window_days).map_err(|source| {
        DumpCommandError::Read {
            path: path.to_path_buf(),
            source,
        }
    })
}

/// The one-line result of `dump fetch`.
pub fn render_outcome(outcome: &FetchOutcome) -> String {
    match outcome {
        FetchOutcome::Unchanged { path } => format!("unchanged: {}", path.display()),
        FetchOutcome::Downloaded { path, bytes } => {
            format!("downloaded: {} ({bytes} bytes)", path.display())
        }
    }
}

/// The `key: value` lines of `dump stats`, in a fixed order.
pub fn render_stats(snapshot: &DumpSnapshot, window_days: u16) -> String {
    let lines = [
        ("dump_timestamp", snapshot.meta.timestamp.to_string()),
        ("dump_commit", snapshot.meta.crates_io_commit.clone()),
        ("dump_directory", snapshot.meta.directory.clone()),
        ("window_days", window_days.to_string()),
        ("crates", snapshot.crates.len().to_string()),
        ("versions", snapshot.versions.len().to_string()),
        (
            "default_versions",
            snapshot.default_version.len().to_string(),
        ),
        ("dependency_edges", snapshot.edges.len().to_string()),
        (
            "crates_with_default_requirements",
            snapshot.default_requirements.len().to_string(),
        ),
        ("crates_with_owners", snapshot.owners.len().to_string()),
        ("users", snapshot.users.len().to_string()),
        ("teams", snapshot.teams.len().to_string()),
        (
            "crates_with_categories",
            snapshot.categories.len().to_string(),
        ),
        ("crates_with_keywords", snapshot.keywords.len().to_string()),
        ("deleted_crates", snapshot.deleted.len().to_string()),
        (
            "crates_with_downloads",
            snapshot.downloads.len().to_string(),
        ),
        (
            "warning_unknown_category",
            snapshot.warnings.unknown_category.to_string(),
        ),
        (
            "warning_unknown_keyword",
            snapshot.warnings.unknown_keyword.to_string(),
        ),
        (
            "warning_unknown_download_version",
            snapshot.warnings.unknown_download_version.to_string(),
        ),
    ];
    lines
        .iter()
        .map(|(key, value)| format!("{key}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use catalogue_signals::dump::DumpMeta;

    use super::*;

    #[test]
    fn outcome_lines_have_a_fixed_format() {
        let unchanged = FetchOutcome::Unchanged {
            path: PathBuf::from("/c/dump/db-dump.tar.gz"),
        };
        assert_eq!(
            render_outcome(&unchanged),
            "unchanged: /c/dump/db-dump.tar.gz"
        );
        let downloaded = FetchOutcome::Downloaded {
            path: PathBuf::from("/c/dump/db-dump.tar.gz"),
            bytes: 12,
        };
        assert_eq!(
            render_outcome(&downloaded),
            "downloaded: /c/dump/db-dump.tar.gz (12 bytes)"
        );
    }

    #[test]
    fn stats_lists_every_key_in_a_fixed_order() {
        let snapshot = DumpSnapshot {
            meta: DumpMeta {
                timestamp: Default::default(),
                crates_io_commit: "abc123".to_owned(),
                directory: "2026-03-02-020017".to_owned(),
            },
            crates: Default::default(),
            versions: Default::default(),
            default_version: Default::default(),
            edges: Vec::new(),
            default_requirements: Default::default(),
            owners: Default::default(),
            users: Default::default(),
            teams: Default::default(),
            categories: Default::default(),
            keywords: Default::default(),
            deleted: Vec::new(),
            downloads: Default::default(),
            warnings: Default::default(),
        };
        let text = render_stats(&snapshot, 90);
        let keys: Vec<&str> = text
            .lines()
            .filter_map(|line| line.split_once(": ").map(|(key, _)| key))
            .collect();
        assert_eq!(
            keys,
            [
                "dump_timestamp",
                "dump_commit",
                "dump_directory",
                "window_days",
                "crates",
                "versions",
                "default_versions",
                "dependency_edges",
                "crates_with_default_requirements",
                "crates_with_owners",
                "users",
                "teams",
                "crates_with_categories",
                "crates_with_keywords",
                "deleted_crates",
                "crates_with_downloads",
                "warning_unknown_category",
                "warning_unknown_keyword",
                "warning_unknown_download_version",
            ]
        );
        assert!(text.contains("dump_commit: abc123"));
        assert!(text.contains("crates: 0"));
    }

    #[test]
    fn missing_dump_names_the_path_and_the_fetch_command() {
        let error = read_cached(Path::new("/nonexistent/db-dump.tar.gz"), 90).expect_err("missing");
        let message = error.to_string();
        assert!(message.contains("/nonexistent/db-dump.tar.gz"), "{message}");
        assert!(message.contains("catalogue dump fetch"), "{message}");
        assert_eq!(error.exit_code(), EXIT_IO);
    }

    #[test]
    fn dump_content_errors_exit_with_1_and_read_errors_with_2() {
        let bad = DumpCommandError::Read {
            path: PathBuf::from("d"),
            source: DumpError::MissingTable { table: "crates" },
        };
        assert_eq!(bad.exit_code(), EXIT_DATA);
        let io = DumpCommandError::Read {
            path: PathBuf::from("d"),
            source: DumpError::Io {
                entry: "archive".to_owned(),
                source: io::Error::other("x"),
            },
        };
        assert_eq!(io.exit_code(), EXIT_IO);
    }
}
