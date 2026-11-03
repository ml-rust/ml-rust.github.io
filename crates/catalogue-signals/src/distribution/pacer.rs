//! Request pacing for the distribution APIs.

use std::time::Duration;

use crate::github::GITHUB_INTERVAL;

/// The shortest gap between two GitHub release requests.
pub const RELEASES_INTERVAL: Duration = GITHUB_INTERVAL;
/// The shortest gap between two Homebrew formula requests.
pub const HOMEBREW_INTERVAL: Duration = Duration::from_secs(1);
/// The shortest gap between two Docker Hub requests.
pub const DOCKER_INTERVAL: Duration = Duration::from_secs(1);
/// The shortest gap between two Repology requests. It is the documented limit.
pub const REPOLOGY_INTERVAL: Duration = Duration::from_secs(1);
