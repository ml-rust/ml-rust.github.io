//! The batched stats requests.

use super::query::{Answer, RepoStats, request_body};
use crate::github::{GRAPHQL_ENDPOINT, GithubError, RepoRef, parse_json};
use crate::http::{Paced, Pacer, Transport};

/// The most repos in one request.
pub const BATCH_SIZE: usize = 80;

/// A request that gave no stats.
#[derive(Debug)]
pub struct FailedBatch {
    /// The repos of the request, in the order asked.
    pub repos: Vec<RepoRef>,
    /// Why the request failed.
    pub error: GithubError,
}

/// The outcome of [`fetch_repo_stats`].
#[derive(Debug, Default)]
pub struct StatsFetch {
    /// The repos GitHub resolved, each with the name that was asked, in the order asked.
    pub found: Vec<(RepoRef, RepoStats)>,
    /// The repos GitHub does not resolve, in the order asked.
    pub unresolved: Vec<RepoRef>,
    /// The requests that failed.
    pub failed: Vec<FailedBatch>,
}

impl StatsFetch {
    /// The repos that no answer covers.
    pub fn unfetched(&self) -> usize {
        self.failed.iter().map(|batch| batch.repos.len()).sum()
    }
}

/// Reads the stats of `repos`, [`BATCH_SIZE`] repos per GraphQL request.
///
/// `token` is sent as a bearer token. `pacer` holds each request back until the API allows it. A
/// 403 or 429 answer with a wait hint is retried. A request that fails for any other reason
/// lands in [`StatsFetch::failed`] and the next batch still runs.
pub fn fetch_repo_stats(
    transport: &dyn Transport,
    token: &str,
    repos: &[RepoRef],
    pacer: &mut dyn Pacer,
) -> StatsFetch {
    let headers = [
        ("Authorization", format!("Bearer {token}")),
        ("Content-Type", "application/json".to_owned()),
    ];
    let mut paced = Paced::new(pacer);
    let mut out = StatsFetch::default();
    for batch in repos.chunks(BATCH_SIZE) {
        match fetch_batch(transport, &headers, &mut paced, batch) {
            Ok(stats) => {
                for (repo, stats) in batch.iter().zip(stats) {
                    match stats {
                        Some(stats) => out.found.push((repo.clone(), stats)),
                        None => out.unresolved.push(repo.clone()),
                    }
                }
            }
            Err(error) => out.failed.push(FailedBatch {
                repos: batch.to_vec(),
                error,
            }),
        }
    }
    out
}

/// One request for `batch`: the stats of each repo, `None` for a repo GitHub does not resolve.
fn fetch_batch(
    transport: &dyn Transport,
    headers: &[(&str, String)],
    paced: &mut Paced<'_>,
    batch: &[RepoRef],
) -> Result<Vec<Option<RepoStats>>, GithubError> {
    let body = request_body(batch);
    let bytes = paced.fetch(GRAPHQL_ENDPOINT, || {
        transport.post(GRAPHQL_ENDPOINT, headers, &body)
    })?;
    let answer: Answer = parse_json(GRAPHQL_ENDPOINT, &bytes)?;
    answer
        .into_parsed(batch.len())
        .map(|parsed| parsed.repos)
        .map_err(|message| GithubError::Graphql {
            repo: describe(batch),
            message,
        })
}

/// A batch as `<n> repos from <first> to <last>`.
fn describe(batch: &[RepoRef]) -> String {
    match (batch.first(), batch.last()) {
        (Some(first), Some(last)) => format!("{} repos from {first} to {last}", batch.len()),
        _ => "an empty batch".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::io::Cursor;

    use super::*;
    use crate::http::{Headers, HttpError, Response};

    /// One canned answer.
    enum Reply {
        Body(String),
        Status(u16),
    }

    /// Replays canned replies and records the posted bodies.
    struct Script {
        replies: RefCell<VecDeque<Reply>>,
        posted: RefCell<Vec<serde_json::Value>>,
        tokens: RefCell<Vec<String>>,
    }

    impl Script {
        fn new(replies: Vec<Reply>) -> Self {
            Self {
                replies: RefCell::new(replies.into()),
                posted: RefCell::new(Vec::new()),
                tokens: RefCell::new(Vec::new()),
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

        fn post(
            &self,
            url: &str,
            headers: &[(&str, String)],
            body: &[u8],
        ) -> Result<Response, HttpError> {
            self.posted
                .borrow_mut()
                .push(serde_json::from_slice(body).expect("json body"));
            self.tokens.borrow_mut().extend(
                headers
                    .iter()
                    .filter(|(name, _)| *name == "Authorization")
                    .map(|(_, value)| value.clone()),
            );
            let reply =
                self.replies
                    .borrow_mut()
                    .pop_front()
                    .ok_or_else(|| HttpError::Request {
                        url: url.to_owned(),
                        source: "no canned reply left".into(),
                    })?;
            let (status, body) = match reply {
                Reply::Body(text) => (200, text),
                Reply::Status(status) => (status, String::new()),
            };
            Ok(Response {
                status,
                headers: Headers::default(),
                body: Box::new(Cursor::new(body.into_bytes())),
            })
        }

        fn patch(&self, url: &str, _: &[(&str, String)], _: &[u8]) -> Result<Response, HttpError> {
            Err(refused(url))
        }

        fn delete(&self, url: &str, _: &[(&str, String)], _: &[u8]) -> Result<Response, HttpError> {
            Err(refused(url))
        }
    }

    /// Lets every request through at once.
    struct NoWait;

    impl Pacer for NoWait {
        fn wait(&mut self, _: &Headers) {}
    }

    fn repos(count: usize) -> Vec<RepoRef> {
        (0..count)
            .map(|index| RepoRef::parse(&format!("o/r{index}")).expect("repo"))
            .collect()
    }

    /// An answer that resolves aliases `0..count` of a batch.
    fn resolving(first_repo: usize, count: usize) -> Reply {
        let nodes: Vec<String> = (0..count)
            .map(|alias| {
                format!(
                    r#""r{alias}":{{"nameWithOwner":"o/r{}","stargazerCount":{}}}"#,
                    first_repo + alias,
                    first_repo + alias
                )
            })
            .collect();
        Reply::Body(format!(r#"{{"data":{{{}}}}}"#, nodes.join(",")))
    }

    fn aliases(body: &serde_json::Value) -> usize {
        body["query"]
            .as_str()
            .expect("query text")
            .matches("...Stats }")
            .count()
    }

    #[test]
    fn repos_split_into_batches_of_at_most_80() {
        let script = Script::new(vec![
            resolving(0, 80),
            resolving(80, 80),
            resolving(160, 10),
        ]);
        let fetched = fetch_repo_stats(&script, "t0k", &repos(170), &mut NoWait);
        let sizes: Vec<usize> = script.posted.borrow().iter().map(aliases).collect();
        assert_eq!(sizes, [80, 80, 10]);
        assert_eq!(fetched.found.len(), 170);
        assert!(fetched.unresolved.is_empty() && fetched.failed.is_empty());
        assert_eq!(fetched.unfetched(), 0);
        let (asked, stats) = &fetched.found[165];
        assert_eq!(asked.to_string(), "o/r165");
        assert_eq!(stats.stars, 165);
        assert!(
            script
                .tokens
                .borrow()
                .iter()
                .all(|token| token == "Bearer t0k")
        );
    }

    #[test]
    fn no_repos_send_no_request() {
        let script = Script::new(Vec::new());
        let fetched = fetch_repo_stats(&script, "t", &[], &mut NoWait);
        assert!(script.posted.borrow().is_empty());
        assert!(fetched.found.is_empty() && fetched.failed.is_empty());
    }

    #[test]
    fn a_repo_that_resolves_to_null_is_unresolved_and_the_others_stay() {
        let script = Script::new(vec![Reply::Body(
            r#"{"data":{"r0":{"nameWithOwner":"o/r0"},"r1":null,"r2":{"nameWithOwner":"o/r2"}},
                "errors":[{"type":"NOT_FOUND","path":["r1"],"message":"gone"}]}"#
                .to_owned(),
        )]);
        let fetched = fetch_repo_stats(&script, "t", &repos(3), &mut NoWait);
        let found: Vec<String> = fetched.found.iter().map(|(r, _)| r.to_string()).collect();
        assert_eq!(found, ["o/r0", "o/r2"]);
        let gone: Vec<String> = fetched.unresolved.iter().map(ToString::to_string).collect();
        assert_eq!(gone, ["o/r1"]);
        assert_eq!(fetched.unfetched(), 0);
    }

    #[test]
    fn a_failed_batch_is_reported_and_the_run_goes_on() {
        let script = Script::new(vec![
            resolving(0, 80),
            Reply::Status(502),
            resolving(160, 5),
        ]);
        let fetched = fetch_repo_stats(&script, "t", &repos(165), &mut NoWait);
        assert_eq!(fetched.found.len(), 85);
        assert_eq!(fetched.failed.len(), 1);
        assert_eq!(fetched.unfetched(), 80);
        let failed = &fetched.failed[0];
        assert_eq!(
            failed.repos.first().map(ToString::to_string).as_deref(),
            Some("o/r80")
        );
        assert_eq!(
            failed.repos.last().map(ToString::to_string).as_deref(),
            Some("o/r159")
        );
        assert!(failed.error.to_string().contains("502"), "{}", failed.error);
    }

    #[test]
    fn a_graphql_error_fails_the_batch_and_names_it() {
        let script = Script::new(vec![Reply::Body(
            r#"{"data":null,"errors":[{"message":"Bad credentials"}]}"#.to_owned(),
        )]);
        let fetched = fetch_repo_stats(&script, "t", &repos(2), &mut NoWait);
        assert!(fetched.found.is_empty());
        assert_eq!(fetched.unfetched(), 2);
        let text = fetched.failed[0].error.to_string();
        assert!(text.contains("2 repos from o/r0 to o/r1"), "{text}");
        assert!(text.contains("Bad credentials"), "{text}");
    }

    #[test]
    fn a_body_that_is_not_json_fails_the_batch() {
        let script = Script::new(vec![Reply::Body("<html>".to_owned())]);
        let fetched = fetch_repo_stats(&script, "t", &repos(1), &mut NoWait);
        assert_eq!(fetched.unfetched(), 1);
        assert!(matches!(fetched.failed[0].error, GithubError::Json { .. }));
    }
}
