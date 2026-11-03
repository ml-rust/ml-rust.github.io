//! Request pacing for the ecosyste.ms API.

use std::time::Duration;

/// The shortest gap between two ecosyste.ms requests.
pub const ECOSYSTEMS_INTERVAL: Duration = Duration::from_secs(1);
