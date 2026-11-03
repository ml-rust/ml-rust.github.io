//! Request pacing for the GitHub API.

use std::time::Duration;

/// The shortest gap between two GitHub requests. The search API, the strictest
/// one the catalogue calls, allows 30 a minute.
pub const GITHUB_INTERVAL: Duration = Duration::from_secs(2);
