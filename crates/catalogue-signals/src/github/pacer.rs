//! Request pacing for the search API.

use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::http::Headers;

/// The shortest gap between two requests. The search API allows 30 a minute.
const MIN_INTERVAL: Duration = Duration::from_secs(2);

/// The longest single wait. It equals the length of a rate-limit window.
const MAX_WAIT: Duration = Duration::from_secs(3600);

/// Holds a request back until the API allows the next one.
pub trait Pacer {
    /// Blocks until the next request is allowed.
    ///
    /// `response_headers` come from the response that precedes the request.
    fn wait(&mut self, response_headers: &Headers);
}

/// A [`Pacer`] that sleeps.
///
/// It waits at least two seconds. It waits for `retry-after` when the header
/// is present. It waits until `x-ratelimit-reset` when `x-ratelimit-remaining`
/// is `0`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SleepPacer;

impl Pacer for SleepPacer {
    fn wait(&mut self, response_headers: &Headers) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        thread::sleep(delay(response_headers, now));
    }
}

/// How long to wait after a response, given the epoch second `now`.
pub(super) fn delay(headers: &Headers, now: u64) -> Duration {
    let mut wait = MIN_INTERVAL;
    if let Some(seconds) = headers
        .get("retry-after")
        .and_then(|value| value.trim().parse::<u64>().ok())
    {
        wait = wait.max(Duration::from_secs(seconds));
    }
    let exhausted = headers
        .get("x-ratelimit-remaining")
        .is_some_and(|value| value.trim() == "0");
    if exhausted
        && let Some(reset) = headers
            .get("x-ratelimit-reset")
            .and_then(|value| value.trim().parse::<u64>().ok())
    {
        wait = wait.max(Duration::from_secs(
            reset.saturating_sub(now).saturating_add(1),
        ));
    }
    wait.min(MAX_WAIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(headers: &[(&str, &str)], now: u64) -> u64 {
        delay(&Headers::new(headers.iter().copied()), now).as_secs()
    }

    #[test]
    fn at_least_two_seconds() {
        assert_eq!(secs(&[], 100), 2);
        assert_eq!(secs(&[("x-ratelimit-remaining", "5")], 100), 2);
    }

    #[test]
    fn retry_after_sets_the_wait() {
        assert_eq!(secs(&[("Retry-After", "17")], 100), 17);
        assert_eq!(secs(&[("retry-after", "1")], 100), 2);
        assert_eq!(secs(&[("retry-after", "soon")], 100), 2);
    }

    #[test]
    fn an_exhausted_window_waits_for_the_reset() {
        let headers = [("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "130")];
        assert_eq!(secs(&headers, 100), 31);
        assert_eq!(secs(&headers, 500), 2);
    }

    #[test]
    fn a_wait_never_exceeds_the_cap() {
        assert_eq!(secs(&[("retry-after", "999999")], 0), 3600);
    }
}
