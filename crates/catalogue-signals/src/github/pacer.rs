//! Request pacing for the GitHub API.

use std::time::Duration;

/// The shortest gap between two GitHub requests. A token has 5,000 requests per
/// hour, and this pace stays under that.
pub const GITHUB_INTERVAL: Duration = Duration::from_secs(1);

/// The shortest gap between two GitHub search requests. The search API allows 30 a minute.
pub const SEARCH_INTERVAL: Duration = Duration::from_secs(2);
