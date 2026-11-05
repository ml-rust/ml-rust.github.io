//! The one way a GraphQL request goes out.

use serde::Deserialize;

use super::error::GithubError;
use super::facts::{ApiError, GRAPHQL_ENDPOINT};
use crate::http::{Paced, Transport};

/// The error type GitHub gives a query over the rate limit.
const RATE_LIMITED: &str = "RATE_LIMITED";

/// The `errors` array of an answer. Every other field is ignored.
#[derive(Deserialize)]
struct Errors {
    #[serde(default)]
    errors: Vec<ApiError>,
}

/// Posts the query `body` and returns the bytes of the answer.
///
/// GitHub answers a query over the rate limit with status 200 and a `RATE_LIMITED` error. That
/// answer takes the retry path of a 403 or 429 with a wait hint: same wait, same retry cap, and
/// [`GithubError::RetriesExhausted`] when the limit stays.
///
/// # Errors
///
/// Returns a [`GithubError`] when no response arrives, the status is not 200, the body breaks
/// off, or the rate limit stays after the retries.
pub(crate) fn post_graphql(
    paced: &mut Paced<'_>,
    transport: &dyn Transport,
    headers: &[(&str, String)],
    body: &[u8],
) -> Result<Vec<u8>, GithubError> {
    let bytes = paced.fetch_checked(
        GRAPHQL_ENDPOINT,
        || transport.post(GRAPHQL_ENDPOINT, headers, body),
        reports_rate_limit,
    )?;
    Ok(bytes)
}

/// Whether the answer `body` holds a `RATE_LIMITED` error. A body that is not the expected JSON
/// holds none, and the caller's parse reports it.
fn reports_rate_limit(body: &[u8]) -> bool {
    serde_json::from_slice::<Errors>(body).is_ok_and(|answer| {
        answer
            .errors
            .iter()
            .any(|error| error.kind.as_deref() == Some(RATE_LIMITED))
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::io::Cursor;
    use std::time::Duration;

    use jiff::civil::date;

    use super::*;
    use crate::github::facts::{RepoFacts, RepoRef, repo_facts};
    use crate::http::{Clock, FakeClock, Headers, HttpError, IntervalPacer, Response};

    const LIMITED: &str =
        r#"{"data":null,"errors":[{"type":"RATE_LIMITED","message":"API rate limit exceeded"}]}"#;
    const FOUND: &str = r#"{"data":{"repository":{"nameWithOwner":"a/b"}}}"#;
    const SPENT: [(&str, &str); 2] = [
        ("x-ratelimit-remaining", "0"),
        ("x-ratelimit-reset", "1500"),
    ];

    /// Replays canned 200 answers, each with its headers, and counts the requests.
    struct Script {
        answers: RefCell<VecDeque<(&'static str, Headers)>>,
        requests: RefCell<u32>,
    }

    impl Script {
        fn new(answers: Vec<(&'static str, Headers)>) -> Self {
            Self {
                answers: RefCell::new(answers.into()),
                requests: RefCell::new(0),
            }
        }
    }

    fn refused(url: &str) -> HttpError {
        HttpError::Request {
            url: url.to_owned(),
            source: "not a GraphQL request".into(),
        }
    }

    impl Transport for Script {
        fn get(&self, url: &str, _: &[(&str, String)]) -> Result<Response, HttpError> {
            Err(refused(url))
        }

        fn post(&self, url: &str, _: &[(&str, String)], _: &[u8]) -> Result<Response, HttpError> {
            *self.requests.borrow_mut() += 1;
            let (body, headers) =
                self.answers
                    .borrow_mut()
                    .pop_front()
                    .ok_or_else(|| HttpError::Request {
                        url: url.to_owned(),
                        source: "no canned answer left".into(),
                    })?;
            Ok(Response {
                status: 200,
                headers,
                body: Box::new(Cursor::new(body.as_bytes().to_vec())),
            })
        }

        fn patch(&self, url: &str, _: &[(&str, String)], _: &[u8]) -> Result<Response, HttpError> {
            Err(refused(url))
        }

        fn delete(&self, url: &str, _: &[(&str, String)], _: &[u8]) -> Result<Response, HttpError> {
            Err(refused(url))
        }
    }

    fn spent() -> Headers {
        Headers::new(SPENT)
    }

    /// The facts of `a/b` from `script`, with a clock that starts at 1000.
    fn facts(script: &Script) -> (Result<RepoFacts, GithubError>, u64) {
        let clock = FakeClock::at(1000);
        let mut pacer = IntervalPacer::with_clock(Duration::from_secs(1), clock.clone());
        let repo = RepoRef::parse("a/b").expect("repo");
        let found = repo_facts(script, "t0k", &repo, date(2026, 11, 3), &mut pacer);
        (found, clock.now())
    }

    #[test]
    fn a_rate_limited_200_waits_for_the_reset_and_retries() {
        let script = Script::new(vec![(LIMITED, spent()), (FOUND, Headers::default())]);
        let (found, now) = facts(&script);
        assert!(matches!(found, Ok(RepoFacts::Found(_))), "{found:?}");
        assert_eq!(now, 1501);
        assert_eq!(*script.requests.borrow(), 2);
    }

    #[test]
    fn the_reset_counts_when_the_remaining_header_is_absent() {
        let reset = Headers::new([("x-ratelimit-reset", "1500")]);
        let script = Script::new(vec![(LIMITED, reset), (FOUND, Headers::default())]);
        let (found, now) = facts(&script);
        assert!(found.is_ok(), "{found:?}");
        assert_eq!(now, 1501);
    }

    #[test]
    fn with_no_reset_header_the_wait_is_the_interval() {
        let script = Script::new(vec![
            (LIMITED, Headers::default()),
            (FOUND, Headers::default()),
        ]);
        let (found, now) = facts(&script);
        assert!(found.is_ok(), "{found:?}");
        assert_eq!(now, 1001);
    }

    #[test]
    fn a_limit_that_stays_ends_in_the_rate_limit_error() {
        let script = Script::new(vec![(LIMITED, Headers::default()); 4]);
        let (found, _) = facts(&script);
        let error = found.expect_err("still limited");
        assert!(
            matches!(error, GithubError::RetriesExhausted { retries: 3, .. }),
            "{error}"
        );
        assert_eq!(*script.requests.borrow(), 4);
        let text = error.to_string();
        assert!(text.contains("rate limit"), "{text}");
        assert!(!text.contains("Check the token"), "{text}");
        assert!(!text.contains("t0k"), "{text}");
    }

    #[test]
    fn another_graphql_error_neither_waits_nor_retries() {
        let other = r#"{"errors":[{"type":"FORBIDDEN","message":"Something went wrong"}]}"#;
        let script = Script::new(vec![(other, spent()), (FOUND, Headers::default())]);
        let (found, now) = facts(&script);
        let error = found.expect_err("rejected");
        assert!(matches!(error, GithubError::Graphql { .. }), "{error}");
        assert_eq!(now, 1000);
        assert_eq!(*script.requests.borrow(), 1);
    }

    #[test]
    fn only_a_rate_limited_error_reports_a_limit() {
        assert!(reports_rate_limit(LIMITED.as_bytes()));
        assert!(!reports_rate_limit(FOUND.as_bytes()));
        assert!(!reports_rate_limit(b"<html>"));
        assert!(!reports_rate_limit(br#"{"errors":[{"type":"NOT_FOUND"}]}"#));
    }
}
