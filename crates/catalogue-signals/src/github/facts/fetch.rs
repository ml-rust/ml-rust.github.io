//! The facts request.

use jiff::civil::Date;

use super::derive::derive_facts;
use super::query::{ENDPOINT, request_body, window_start};
use super::repo_ref::RepoRef;
use super::response::{ApiError, Envelope};
use super::types::RepoFacts;
use crate::github::error::GithubError;
use crate::github::json::parse_json;
use crate::http::{Paced, Pacer, Transport};

/// The error type GitHub gives a name it cannot resolve.
const NOT_FOUND: &str = "NOT_FOUND";

/// The path key of the repository field.
const REPOSITORY_PATH: &str = "repository";

/// Reads the facts of `repo` as of `today` with one GraphQL request.
///
/// `token` is sent as a bearer token. `pacer` holds the request back until the
/// API allows it. A 403 or 429 answer with a wait hint is retried.
///
/// # Errors
///
/// Returns a [`GithubError`] when the request fails, the server answers with
/// an error status, the body is not the expected JSON, the rate limit does not
/// lift after the retries, or the API reports a GraphQL error other than a
/// missing repo.
pub fn repo_facts(
    transport: &dyn Transport,
    token: &str,
    repo: &RepoRef,
    today: Date,
    pacer: &mut dyn Pacer,
) -> Result<RepoFacts, GithubError> {
    let body = request_body(repo, window_start(today));
    let headers = [
        ("Authorization", format!("Bearer {token}")),
        ("Content-Type", "application/json".to_owned()),
    ];
    let mut paced = Paced::new(pacer);
    let bytes = paced.fetch(ENDPOINT, || transport.post(ENDPOINT, &headers, &body))?;
    let envelope: Envelope = parse_json(ENDPOINT, &bytes)?;
    interpret(envelope, repo, today)
}

/// Whether `error` reports that the repository does not exist.
fn is_missing_repository(error: &ApiError) -> bool {
    error.kind.as_deref() == Some(NOT_FOUND)
        && error
            .path
            .first()
            .is_some_and(|key| key.as_str() == Some(REPOSITORY_PATH))
}

/// Turns an answer into facts, or into the first error it holds.
fn interpret(envelope: Envelope, repo: &RepoRef, today: Date) -> Result<RepoFacts, GithubError> {
    let graphql = |message: String| GithubError::Graphql {
        repo: repo.to_string(),
        message,
    };
    let other: Vec<&str> = envelope
        .errors
        .iter()
        .filter(|error| !is_missing_repository(error))
        .map(|error| error.message.as_str())
        .collect();
    if !other.is_empty() {
        return Err(graphql(other.join("; ")));
    }
    let Some(data) = envelope.data else {
        if envelope.errors.is_empty() {
            return Err(graphql("the answer holds no data".to_owned()));
        }
        return Ok(RepoFacts::Gone {
            requested: repo.clone(),
        });
    };
    Ok(match data.repository {
        Some(repository) => RepoFacts::Found(derive_facts(&repository, repo, today)),
        None => RepoFacts::Gone {
            requested: repo.clone(),
        },
    })
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    fn run(json: &str) -> Result<RepoFacts, GithubError> {
        let envelope: Envelope = serde_json::from_str(json).expect("envelope");
        let repo = RepoRef::parse("a/b").expect("parses");
        interpret(envelope, &repo, date(2026, 11, 3))
    }

    #[test]
    fn a_not_found_error_on_the_repository_is_gone() {
        let gone = run(
            r#"{"data":{"repository":null},"errors":[{"type":"NOT_FOUND","path":["repository"],"message":"x"}]}"#,
        )
        .expect("gone");
        assert!(matches!(gone, RepoFacts::Gone { .. }));
    }

    #[test]
    fn a_null_repository_without_errors_is_gone() {
        let gone = run(r#"{"data":{"repository":null}}"#).expect("gone");
        assert!(matches!(gone, RepoFacts::Gone { .. }));
    }

    #[test]
    fn other_errors_name_the_repo_and_the_messages() {
        let error = run(
            r#"{"data":null,"errors":[{"type":"NOT_FOUND","path":["x"],"message":"one"},{"message":"two"}]}"#,
        )
        .expect_err("graphql");
        let text = error.to_string();
        assert!(text.contains("a/b") && text.contains("one; two"), "{text}");
        assert!(text.contains("then rerun"), "{text}");
    }

    #[test]
    fn an_answer_with_no_data_and_no_errors_is_an_error() {
        let error = run(r#"{}"#).expect_err("no data");
        assert!(error.to_string().contains("holds no data"));
    }
}
