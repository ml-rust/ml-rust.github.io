//! The `catalogue distribution` command: release, Homebrew, Docker Hub and
//! Repology counts for apps.

mod command;
mod lookup;
mod render;

pub use command::{DistributionCommand, DistributionCommandError, run_distribution};
pub(crate) use lookup::split_docker_name;
pub use render::{render_docker, render_homebrew, render_releases, render_repology};
