//! Distribution signals: how many people install an app.
//!
//! [`release_downloads`] reads the binary download counts of the latest GitHub
//! releases. [`homebrew_installs`] reads the Homebrew install analytics of a
//! formula. [`homebrew_source_links`] reads the links that name its source.
//! [`docker_pulls`] reads the pull count of a Docker Hub repo.
//! [`repology_packaging`] reads which distribution repos package a project.
//! Each function takes a [`Pacer`](crate::http::Pacer) set to the interval
//! const of its API and gives `None` when the API does not know the name. The
//! HTTP layer sits behind [`Transport`](crate::http::Transport), so every
//! lookup runs without a network.
//!
//! Docker Hub gives the current pull count only. Growth needs two snapshots.

mod assets;
mod docker;
mod error;
mod family;
mod homebrew;
mod json;
mod pacer;
mod releases;
mod repology;

pub use assets::NON_BINARY_SUFFIXES;
pub use docker::{DockerPulls, docker_pulls};
pub use error::DistributionError;
pub use family::family;
pub use homebrew::{HomebrewInstalls, homebrew_installs, homebrew_source_links};
pub use pacer::{DOCKER_INTERVAL, HOMEBREW_INTERVAL, RELEASES_INTERVAL, REPOLOGY_INTERVAL};
pub use releases::{RELEASE_SAMPLE, ReleaseDownloads, ReleaseSummary, release_downloads};
pub use repology::{NON_DISTRIBUTION_REPOS, RepologyPackaging, repology_packaging};
