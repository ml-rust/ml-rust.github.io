//! The GitHub lookup of a release commit in the refs of a repo.

use serde::de::DeserializeOwned;

use super::query::{
    Answer, CommitLookup, ComparedRef, NamedRefs, RefPage, TagRef, branches_body, commit_body,
    named_refs_body, tags_body,
};
use super::types::CrateRelease;
use crate::github::{
    ApiError, GRAPHQL_ENDPOINT, GithubError, NOT_FOUND, RepoRef, is_missing_repository, parse_json,
};
use crate::http::{Paced, Pacer, Transport};

/// The most pages read per branch or tag scan, at 100 refs per page.
const MAX_REF_PAGES: usize = 50;

/// The path key of the comparison field.
const COMPARE_PATH: &str = "compare";

/// Where a commit stands against the refs of a repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitStanding {
    /// A branch or tag of the repo holds the commit.
    Held {
        /// The full ref name, such as `refs/tags/v1.2.3`.
        held_by: String,
    },
    /// GitHub knows the commit, but no branch or tag of the repo holds it. A
    /// commit that only a fork holds stands here.
    NotHeld,
    /// GitHub does not know the commit or the repo.
    Missing,
}

/// Asks GitHub whether a branch or tag of `repo` holds commit `sha1` of `release`.
///
/// GitHub serves a commit of any fork through the parent repo. A commit that
/// resolves proves nothing, so a ref of `repo` must contain it. Only a person
/// with write access to `repo` can create such a ref. Pull request refs do not
/// count, because a fork author can create them.
///
/// The lookup stops at the first ref that holds the commit:
///
/// 1. One request checks that GitHub knows the commit.
/// 2. One request compares it with the default branch and with the tags
///    `v<version>`, `<version>`, `<crate>-v<version>` and `<crate>-<version>`.
/// 3. One request per 100 branches compares it with every branch. A branch
///    name proves nothing, so no name filter applies.
/// 4. One request per 100 tags looks for a tag that points straight at it.
///
/// A repo with more than 5000 branches or tags is scanned up to that count.
/// A commit past it stands as [`CommitStanding::NotHeld`].
///
/// `token` is sent as a bearer token. A 403 or 429 answer with a wait hint is
/// retried after the `pacer` waits.
///
/// # Errors
///
/// Returns a [`GithubError`] when no response arrives, a status is not 200, a
/// body breaks off or is not the expected JSON, the rate limit stays after the
/// retries, or the API reports a GraphQL error other than a missing repo or an
/// unresolved comparison.
pub fn commit_standing(
    transport: &dyn Transport,
    token: &str,
    repo: &RepoRef,
    release: &CrateRelease,
    sha1: &str,
    pacer: &mut dyn Pacer,
) -> Result<CommitStanding, GithubError> {
    let mut lookup = Lookup {
        transport,
        headers: [
            ("Authorization", format!("Bearer {token}")),
            ("Content-Type", "application/json".to_owned()),
        ],
        paced: Paced::new(pacer),
        repo,
    };
    let commit: Option<CommitLookup> = lookup.ask(&commit_body(repo, sha1))?;
    if !commit.is_some_and(|found| found.is_commit()) {
        return Ok(CommitStanding::Missing);
    }
    let named: Option<NamedRefs> = lookup.ask(&named_refs_body(repo, release, sha1))?;
    let mut holder = named.and_then(|refs| refs.holder());
    if holder.is_none() {
        holder = lookup.scan(
            |after| branches_body(repo, sha1, after),
            ComparedRef::holding_branch,
        )?;
    }
    if holder.is_none() {
        holder = lookup.scan(
            |after| tags_body(repo, after),
            |tag: &TagRef| tag.holding(sha1),
        )?;
    }
    Ok(
        holder.map_or(CommitStanding::NotHeld, |held_by| CommitStanding::Held {
            held_by,
        }),
    )
}

/// The requests of one lookup.
struct Lookup<'a, 'p> {
    transport: &'a dyn Transport,
    headers: [(&'static str, String); 2],
    paced: Paced<'p>,
    repo: &'a RepoRef,
}

impl Lookup<'_, '_> {
    /// Sends one query and returns its `repository` object.
    fn ask<T: DeserializeOwned>(&mut self, body: &[u8]) -> Result<Option<T>, GithubError> {
        let (transport, headers) = (self.transport, &self.headers);
        let bytes = self.paced.fetch(GRAPHQL_ENDPOINT, || {
            transport.post(GRAPHQL_ENDPOINT, headers, body)
        })?;
        let answer: Answer<T> = parse_json(GRAPHQL_ENDPOINT, &bytes)?;
        interpret(answer, self.repo)
    }

    /// Reads pages of refs until `holding` names a ref that holds the commit.
    fn scan<T: DeserializeOwned>(
        &mut self,
        body: impl Fn(Option<&str>) -> Vec<u8>,
        holding: impl Fn(&T) -> Option<String>,
    ) -> Result<Option<String>, GithubError> {
        let mut after: Option<String> = None;
        for _ in 0..MAX_REF_PAGES {
            let page: Option<RefPage<T>> = self.ask(&body(after.as_deref()))?;
            let Some(refs) = page.and_then(|page| page.refs) else {
                return Ok(None);
            };
            if let Some(holder) = refs.items().find_map(&holding) {
                return Ok(Some(holder));
            }
            after = refs.next_cursor();
            if after.is_none() {
                return Ok(None);
            }
        }
        Ok(None)
    }
}

/// Whether `error` reports a comparison GitHub cannot resolve. It proves nothing,
/// so the ref does not hold the commit.
fn is_unresolved_comparison(error: &ApiError) -> bool {
    error.kind.as_deref() == Some(NOT_FOUND)
        && error
            .path
            .last()
            .is_some_and(|key| key.as_str() == Some(COMPARE_PATH))
}

/// The `repository` object of an answer, or the errors it holds.
fn interpret<T>(answer: Answer<T>, repo: &RepoRef) -> Result<Option<T>, GithubError> {
    let graphql = |message: String| GithubError::Graphql {
        repo: repo.to_string(),
        message,
    };
    let other: Vec<&str> = answer
        .errors
        .iter()
        .filter(|error| !is_missing_repository(error) && !is_unresolved_comparison(error))
        .map(|error| error.message.as_str())
        .collect();
    if !other.is_empty() {
        return Err(graphql(other.join("; ")));
    }
    match answer.data {
        Some(data) => Ok(data.repository),
        None if answer.errors.is_empty() => Err(graphql("the answer holds no data".to_owned())),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::io::Cursor;

    use super::*;
    use crate::http::{Headers, HttpError, Response};

    const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";

    const COMMIT: &str = r#"{"data":{"repository":{"object":{"__typename":"Commit"}}}}"#;

    /// Replays canned bodies and records the posted queries.
    struct Script {
        answers: RefCell<VecDeque<String>>,
        posted: RefCell<Vec<serde_json::Value>>,
    }

    impl Script {
        fn new(answers: &[&str]) -> Self {
            Self {
                answers: RefCell::new(answers.iter().map(|&text| text.to_owned()).collect()),
                posted: RefCell::new(Vec::new()),
            }
        }

        fn queries(&self) -> Vec<String> {
            self.posted
                .borrow()
                .iter()
                .map(|body| body["query"].as_str().expect("query").to_owned())
                .collect()
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
            _: &[(&str, String)],
            body: &[u8],
        ) -> Result<Response, HttpError> {
            self.posted
                .borrow_mut()
                .push(serde_json::from_slice(body).expect("json body"));
            let answer =
                self.answers
                    .borrow_mut()
                    .pop_front()
                    .ok_or_else(|| HttpError::Request {
                        url: url.to_owned(),
                        source: "no canned answer left".into(),
                    })?;
            Ok(Response {
                status: 200,
                headers: Headers::default(),
                body: Box::new(Cursor::new(answer.into_bytes())),
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

    fn compared(name: &str, status: &str) -> String {
        format!(r#"{{"name":"{name}","compare":{{"status":"{status}"}}}}"#)
    }

    /// The default branch and tag answer: `main` with `main_status`, `tag0` as given.
    fn named(main_status: &str, tag0: Option<&str>) -> String {
        let main = compared("main", main_status);
        let tag0 = tag0.map_or_else(|| "null".to_owned(), str::to_owned);
        format!(
            r#"{{"data":{{"repository":{{"defaultBranchRef":{main},"tag0":{tag0},"tag1":null,"tag2":null,"tag3":null}}}}}}"#
        )
    }

    fn page(nodes: &[String], next: Option<&str>) -> String {
        let info = match next {
            Some(cursor) => format!(r#"{{"hasNextPage":true,"endCursor":"{cursor}"}}"#),
            None => r#"{"hasNextPage":false,"endCursor":null}"#.to_owned(),
        };
        format!(
            r#"{{"data":{{"repository":{{"refs":{{"pageInfo":{info},"nodes":[{}]}}}}}}}}"#,
            nodes.join(",")
        )
    }

    fn lookup(script: &Script) -> Result<CommitStanding, GithubError> {
        let repo = RepoRef::parse("tracel-ai/burn").expect("repo");
        let release = CrateRelease {
            name: "burn".to_owned(),
            version: "0.22.1".to_owned(),
        };
        commit_standing(script, "t0k", &repo, &release, SHA, &mut NoWait)
    }

    fn held(name: &str) -> CommitStanding {
        CommitStanding::Held {
            held_by: name.to_owned(),
        }
    }

    #[test]
    fn a_commit_on_the_default_branch_stops_after_two_requests() {
        for status in ["BEHIND", "IDENTICAL"] {
            let script = Script::new(&[COMMIT, &named(status, None)]);
            assert_eq!(lookup(&script).expect("looks up"), held("refs/heads/main"));
            assert_eq!(script.queries().len(), 2, "{status}");
        }
    }

    #[test]
    fn a_commit_on_a_version_tag_stops_after_two_requests() {
        let tag = compared("v0.22.1", "IDENTICAL");
        let script = Script::new(&[COMMIT, &named("DIVERGED", Some(&tag))]);
        assert_eq!(
            lookup(&script).expect("looks up"),
            held("refs/tags/v0.22.1")
        );
        assert_eq!(script.queries().len(), 2);
    }

    #[test]
    fn a_commit_on_a_later_page_of_branches_is_found_and_no_tag_scan_runs() {
        let first = page(&[compared("a", "DIVERGED")], Some("c1"));
        let second = page(&[compared("feat/x", "BEHIND")], None);
        let script = Script::new(&[COMMIT, &named("DIVERGED", None), &first, &second]);
        assert_eq!(
            lookup(&script).expect("looks up"),
            held("refs/heads/feat/x")
        );
        let posted = script.posted.borrow();
        assert_eq!(posted.len(), 4);
        assert_eq!(posted[2]["variables"]["after"], serde_json::Value::Null);
        assert_eq!(posted[3]["variables"]["after"], "c1");
    }

    #[test]
    fn a_tag_with_another_name_that_points_at_the_commit_holds_it() {
        let branches = page(&[compared("main", "DIVERGED")], None);
        let tags = page(
            &[format!(
                r#"{{"name":"burn/v0.22.1","target":{{"oid":"ffff","target":{{"oid":"{SHA}"}}}}}}"#
            )],
            None,
        );
        let script = Script::new(&[COMMIT, &named("DIVERGED", None), &branches, &tags]);
        assert_eq!(
            lookup(&script).expect("looks up"),
            held("refs/tags/burn/v0.22.1")
        );
    }

    /// A fork commit resolves through the parent. It compares as `AHEAD` when
    /// its branch starts at the parent head and `DIVERGED` otherwise.
    #[test]
    fn a_fork_network_commit_never_verifies() {
        for status in ["AHEAD", "DIVERGED"] {
            let tag = compared("v0.22.1", status);
            let branches = page(
                &[compared("main", status), compared("release/0.22", status)],
                None,
            );
            let tags = page(
                &[r#"{"name":"v0.22.1","target":{"oid":"ffff"}}"#.to_owned()],
                None,
            );
            let script = Script::new(&[COMMIT, &named(status, Some(&tag)), &branches, &tags]);
            assert_eq!(
                lookup(&script).expect("looks up"),
                CommitStanding::NotHeld,
                "{status}"
            );
            let queries = script.queries();
            assert_eq!(queries.len(), 4, "{status}");
            assert!(queries[2].contains("refs/heads/"), "{status}");
            assert!(queries[3].contains("refs/tags/"), "{status}");
            assert!(
                queries.iter().all(|query| !query.contains("refs/pull")),
                "{status}"
            );
        }
    }

    #[test]
    fn an_unknown_commit_is_missing_and_sends_no_compare() {
        let answer = r#"{"data":{"repository":{"object":null}}}"#;
        let script = Script::new(&[answer]);
        assert_eq!(lookup(&script).expect("looks up"), CommitStanding::Missing);
        assert_eq!(script.queries().len(), 1);
    }

    #[test]
    fn an_object_that_is_not_a_commit_is_missing() {
        let answer = r#"{"data":{"repository":{"object":{"__typename":"Blob"}}}}"#;
        let script = Script::new(&[answer]);
        assert_eq!(lookup(&script).expect("looks up"), CommitStanding::Missing);
    }

    #[test]
    fn an_unknown_repo_is_missing() {
        let answer = r#"{"data":{"repository":null},"errors":[{"type":"NOT_FOUND","path":["repository"],"message":"gone"}]}"#;
        let script = Script::new(&[answer]);
        assert_eq!(lookup(&script).expect("looks up"), CommitStanding::Missing);
    }

    #[test]
    fn an_unresolved_comparison_does_not_hold_the_commit() {
        let branches = r#"{"data":{"repository":{"refs":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[{"name":"a","compare":null}]}}},"errors":[{"type":"NOT_FOUND","path":["repository","refs","nodes",0,"compare"],"message":"no head"}]}"#;
        let tags = page(&[], None);
        let script = Script::new(&[COMMIT, &named("DIVERGED", None), branches, &tags]);
        assert_eq!(lookup(&script).expect("looks up"), CommitStanding::NotHeld);
    }

    #[test]
    fn a_graphql_error_names_the_repo_and_never_verifies() {
        let answer = r#"{"errors":[{"message":"Something went wrong"}]}"#;
        let script = Script::new(&[COMMIT, answer]);
        let error = lookup(&script).expect_err("rejected");
        assert!(matches!(error, GithubError::Graphql { .. }), "{error}");
        let text = error.to_string();
        assert!(text.contains("tracel-ai/burn"), "{text}");
        assert!(text.contains("Something went wrong"), "{text}");
        assert!(!text.contains("t0k"), "{text}");
    }

    #[test]
    fn an_answer_with_no_data_and_no_error_is_an_error() {
        let script = Script::new(&["{}"]);
        let error = lookup(&script).expect_err("rejected");
        assert!(error.to_string().contains("holds no data"), "{error}");
    }

    #[test]
    fn a_scan_stops_at_the_page_limit_and_does_not_verify() {
        let full = page(&[compared("a", "DIVERGED")], Some("next"));
        let tags = page(&[], None);
        let mut answers = vec![COMMIT.to_owned(), named("DIVERGED", None)];
        answers.extend(std::iter::repeat_n(full, MAX_REF_PAGES));
        answers.push(tags);
        let answers: Vec<&str> = answers.iter().map(String::as_str).collect();
        let script = Script::new(&answers);
        assert_eq!(lookup(&script).expect("looks up"), CommitStanding::NotHeld);
        assert_eq!(script.queries().len(), 2 + MAX_REF_PAGES + 1);
    }
}
