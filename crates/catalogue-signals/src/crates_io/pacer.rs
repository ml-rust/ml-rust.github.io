//! Request pacing for the crates.io API.

use std::time::Duration;

/// The shortest gap between two crates.io API requests, as the crates.io crawler policy asks.
pub const CRATES_IO_INTERVAL: Duration = Duration::from_secs(1);
