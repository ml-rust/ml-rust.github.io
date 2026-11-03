//! Request pacing shared by every rate-limited API.

use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::headers::Headers;

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
/// It waits at least `min_interval`. It waits for `retry-after` when the header
/// is present. It waits until `x-ratelimit-reset` when `x-ratelimit-remaining`
/// is `0`.
#[derive(Debug, Clone, Copy)]
pub struct IntervalPacer {
    min_interval: Duration,
}

impl IntervalPacer {
    /// A pacer that leaves `min_interval` between two requests.
    #[must_use]
    pub const fn new(min_interval: Duration) -> Self {
        Self { min_interval }
    }
}

impl Pacer for IntervalPacer {
    fn wait(&mut self, response_headers: &Headers) {
        sleep(response_headers, self.min_interval);
    }
}

/// Sleeps for [`delay`], measured from the current time.
fn sleep(headers: &Headers, min_interval: Duration) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    thread::sleep(delay(headers, now, min_interval));
}

/// How long to wait after a response, given the epoch second `now`.
///
/// The wait is at least `min_interval`. It lasts until `retry-after` when the
/// header holds a number of seconds. It lasts until `x-ratelimit-reset` when
/// `x-ratelimit-remaining` is `0`. One window caps it.
fn delay(headers: &Headers, now: u64, min_interval: Duration) -> Duration {
    let mut wait = min_interval;
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

    fn secs(headers: &[(&str, &str)], now: u64, min: u64) -> u64 {
        delay(
            &Headers::new(headers.iter().copied()),
            now,
            Duration::from_secs(min),
        )
        .as_secs()
    }

    #[test]
    fn at_least_the_minimum_interval() {
        assert_eq!(secs(&[], 100, 2), 2);
        assert_eq!(secs(&[], 100, 1), 1);
        assert_eq!(secs(&[("x-ratelimit-remaining", "5")], 100, 2), 2);
    }

    #[test]
    fn retry_after_sets_the_wait() {
        assert_eq!(secs(&[("Retry-After", "17")], 100, 2), 17);
        assert_eq!(secs(&[("retry-after", "1")], 100, 2), 2);
        assert_eq!(secs(&[("retry-after", "soon")], 100, 2), 2);
    }

    #[test]
    fn an_exhausted_window_waits_for_the_reset() {
        let headers = [("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "130")];
        assert_eq!(secs(&headers, 100, 2), 31);
        assert_eq!(secs(&headers, 500, 2), 2);
    }

    #[test]
    fn a_wait_never_exceeds_the_cap() {
        assert_eq!(secs(&[("retry-after", "999999")], 0, 2), 3600);
    }
}
