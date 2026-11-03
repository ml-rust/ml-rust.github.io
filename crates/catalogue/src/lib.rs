//! Command line front end for the ml-rust catalogue data files.

pub mod cache_dir;
pub mod cli;
pub mod data_dir;
pub mod discover;
pub mod dump;
mod exit;
pub mod usage;
pub mod validate;

pub use cache_dir::{CacheDirError, default_cache_dir, resolve_cache_dir};
pub use cli::{Cli, Command, run};
pub use data_dir::{DataDir, DataDirError, read_data_dir};
pub use discover::{
    DiscoverCommand, DiscoverError, GithubReport, Report, build_github_report, build_report,
    run_discover,
};
pub use dump::{DumpCommand, DumpCommandError, render_outcome, render_stats, run_dump};
pub use usage::{render_usage, resolve_names};
pub use validate::{Outcome, Summary, validate};
