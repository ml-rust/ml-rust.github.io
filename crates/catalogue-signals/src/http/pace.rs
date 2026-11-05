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

    /// Reads the headers of a response, whatever its status. A pacer with no state ignores them.
    fn observe(&mut self, _response_headers: &Headers) {}

    /// Blocks until a limit that an observed response reported has passed. A pacer with no state
    /// sends at once. [`Paced`](super::Paced) calls this before every request.
    fn hold(&mut self) {}
}

/// The time source of an [`IntervalPacer`].
pub trait Clock {
    /// The current epoch second.
    fn now(&self) -> u64;

    /// Blocks for `wait`.
    fn sleep(&self, wait: Duration);
}

/// The system time and real sleeps.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs())
    }

    fn sleep(&self, wait: Duration) {
        thread::sleep(wait);
    }
}

/// A [`Pacer`] that sleeps.
///
/// It waits at least `min_interval`. It waits for `retry-after` when the header
/// is present. It waits until `x-ratelimit-reset` when `x-ratelimit-remaining`
/// is `0`.
///
/// It remembers those limits from every observed response, so one pacer value carries them from
/// one lookup to the next. A command shares one value across all requests to one API.
#[derive(Debug, Clone)]
pub struct IntervalPacer<C = SystemClock> {
    min_interval: Duration,
    /// The epoch second from which the API allows the next request.
    ready_at: u64,
    clock: C,
}

impl IntervalPacer {
    /// A pacer that leaves `min_interval` between two requests.
    #[must_use]
    pub const fn new(min_interval: Duration) -> Self {
        Self::with_clock(min_interval, SystemClock)
    }
}

impl<C> IntervalPacer<C> {
    /// A pacer that reads the time from `clock` and sleeps on it.
    #[must_use]
    pub const fn with_clock(min_interval: Duration, clock: C) -> Self {
        Self {
            min_interval,
            ready_at: 0,
            clock,
        }
    }
}

impl<C: Clock> Pacer for IntervalPacer<C> {
    fn wait(&mut self, response_headers: &Headers) {
        let wait = delay(response_headers, self.clock.now(), self.min_interval);
        self.clock.sleep(wait);
    }

    fn observe(&mut self, response_headers: &Headers) {
        let now = self.clock.now();
        let wait = delay(response_headers, now, Duration::ZERO);
        self.ready_at = self.ready_at.max(now.saturating_add(wait.as_secs()));
    }

    fn hold(&mut self) {
        let left = self.ready_at.saturating_sub(self.clock.now());
        if left > 0 {
            self.clock.sleep(Duration::from_secs(left));
        }
    }
}

/// A [`Pacer`] for requests to another host that share the spacing of an inner pacer.
///
/// It passes [`Pacer::wait`] on. It drops [`Pacer::observe`] and [`Pacer::hold`], so the budget of
/// this host and the budget the inner pacer tracks never hold each other's requests.
pub struct SpacingOnly<'a>(pub &'a mut dyn Pacer);

impl Pacer for SpacingOnly<'_> {
    fn wait(&mut self, response_headers: &Headers) {
        self.0.wait(response_headers);
    }
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

/// A [`Clock`] whose sleeps move its time and take no real time. Clones share the time.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct FakeClock(std::rc::Rc<std::cell::Cell<u64>>);

#[cfg(test)]
impl FakeClock {
    /// A clock at the epoch second `now`.
    pub(crate) fn at(now: u64) -> Self {
        Self(std::rc::Rc::new(std::cell::Cell::new(now)))
    }
}

#[cfg(test)]
impl Clock for FakeClock {
    fn now(&self) -> u64 {
        self.0.get()
    }

    fn sleep(&self, wait: Duration) {
        self.0.set(self.0.get() + wait.as_secs());
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::super::error::HttpError;
    use super::super::paced::Paced;
    use super::super::transport::Response;
    use super::*;

    fn answer(status: u16, headers: &[(&str, &str)]) -> Result<Response, HttpError> {
        Ok(Response {
            status,
            headers: Headers::new(headers.iter().copied()),
            body: Box::new(Cursor::new(b"{}".to_vec())),
        })
    }

    const SPENT: [(&str, &str); 2] = [
        ("x-ratelimit-remaining", "0"),
        ("x-ratelimit-reset", "1500"),
    ];

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

    #[test]
    fn a_spent_budget_on_a_200_holds_the_next_request_of_a_later_lookup() {
        let clock = FakeClock::at(1000);
        let mut pacer = IntervalPacer::with_clock(Duration::from_secs(1), clock.clone());
        Paced::new(&mut pacer)
            .fetch("u", || answer(200, &SPENT))
            .expect("first lookup");
        assert_eq!(clock.now(), 1000, "the answer itself causes no wait");
        let mut sent_at = None;
        Paced::new(&mut pacer)
            .fetch("u", || {
                sent_at = Some(clock.now());
                answer(200, &[])
            })
            .expect("second lookup");
        assert_eq!(sent_at, Some(1501));
    }

    #[test]
    fn a_spent_budget_on_any_status_is_remembered() {
        for status in [404, 500] {
            let clock = FakeClock::at(1000);
            let mut pacer = IntervalPacer::with_clock(Duration::from_secs(1), clock.clone());
            let result = Paced::new(&mut pacer).fetch("u", || answer(status, &SPENT));
            assert!(result.is_err(), "{status}");
            Paced::new(&mut pacer)
                .fetch("u", || answer(200, &[]))
                .expect("second lookup");
            assert_eq!(clock.now(), 1501, "{status}");
        }
    }

    #[test]
    fn a_wait_inside_one_lookup_is_not_repeated_by_the_hold() {
        let clock = FakeClock::at(1000);
        let mut pacer = IntervalPacer::with_clock(Duration::from_secs(1), clock.clone());
        let mut paced = Paced::new(&mut pacer);
        paced.fetch("u", || answer(200, &SPENT)).expect("first");
        paced.fetch("u", || answer(200, &[])).expect("second");
        assert_eq!(clock.now(), 1501);
    }

    #[test]
    fn a_budget_with_requests_left_causes_no_hold() {
        let clock = FakeClock::at(1000);
        let mut pacer = IntervalPacer::with_clock(Duration::from_secs(1), clock.clone());
        let headers = [
            ("x-ratelimit-remaining", "7"),
            ("x-ratelimit-reset", "1500"),
        ];
        Paced::new(&mut pacer)
            .fetch("u", || answer(200, &headers))
            .expect("first");
        Paced::new(&mut pacer)
            .fetch("u", || answer(200, &[]))
            .expect("second");
        assert_eq!(clock.now(), 1000);
    }

    #[test]
    fn spacing_only_passes_waits_on_and_never_remembers_a_budget() {
        let clock = FakeClock::at(1000);
        let mut pacer = IntervalPacer::with_clock(Duration::from_secs(1), clock.clone());
        let mut other_host = SpacingOnly(&mut pacer);
        Paced::new(&mut other_host)
            .fetch("u", || answer(200, &SPENT))
            .expect("first");
        other_host.wait(&Headers::default());
        assert_eq!(clock.now(), 1001);
        Paced::new(&mut pacer)
            .fetch("u", || answer(200, &[]))
            .expect("the inner pacer holds nothing");
        assert_eq!(clock.now(), 1001);
    }
}
