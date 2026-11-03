//! Command line front end for the ml-rust catalogue data files.

mod batch;
pub mod cache_dir;
pub mod cli;
pub mod data_dir;
pub mod discover;
pub mod distribution;
pub mod dump;
pub mod ecosystems;
mod exit;
pub mod github;
mod render;
pub mod rustsec;
pub mod tiers;
mod token;
pub mod usage;
pub mod validate;

pub use cache_dir::{CacheDirError, default_cache_dir, resolve_cache_dir};
pub use cli::{Cli, Command, run};
pub use data_dir::{DataDir, DataDirError, load_catalogue, read_data_dir};
pub use discover::{
    DiscoverCommand, DiscoverError, GithubReport, Report, build_github_report, build_report,
    run_discover,
};
pub use distribution::{
    DistributionCommand, DistributionCommandError, render_docker, render_homebrew, render_releases,
    render_repology, run_distribution,
};
pub use dump::{DumpCommand, DumpCommandError, render_outcome, render_stats, run_dump};
pub use ecosystems::{EcosystemsCommand, EcosystemsCommandError, run_ecosystems};
pub use github::{GithubCommand, GithubCommandError, render_all, render_facts, run_github};
pub use rustsec::{RustsecCommand, RustsecCommandError, render_crate, run_rustsec};
pub use tiers::{Network, TiersArgs, TiersError, run_tiers, tiers_report};
pub use token::NoToken;
pub use usage::{render_usage, resolve_names};
pub use validate::{Outcome, Summary, validate};
