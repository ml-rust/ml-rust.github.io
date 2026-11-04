#[path = "common/recording_pacer.rs"]
mod recording_pacer;
#[path = "common/request_headers.rs"]
mod request_headers;
#[path = "common/request_urls.rs"]
mod request_urls;
#[path = "common/response_status.rs"]
mod response_status;
#[path = "common/search_responses.rs"]
mod search_responses;
#[path = "common/transport.rs"]
mod transport;

use catalogue_signals::github::{GithubError, RepoRef};
use catalogue_signals::link::{CommitStanding, CrateRelease, commit_standing};
use recording_pacer::RecordingPacer;
use transport::{FakeResponse, FakeTransport};

const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";

const GRAPHQL: &str = "https://api.github.com/graphql";

fn known() -> FakeResponse {
    FakeResponse::with_body(
        200,
        &[],
        r#"{"data":{"repository":{"object":{"__typename":"Commit"}}}}"#,
    )
}

fn on_default(status: &str) -> FakeResponse {
    FakeResponse::with_body(
        200,
        &[],
        &format!(
            r#"{{"data":{{"repository":{{"defaultBranchRef":{{"name":"master","compare":{{"status":"{status}"}}}},"tag0":null,"tag1":null,"tag2":null,"tag3":null}}}}}}"#
        ),
    )
}

fn release() -> CrateRelease {
    CrateRelease {
        name: "serde".to_owned(),
        version: "1.0.228".to_owned(),
    }
}

fn lookup(responses: Vec<FakeResponse>) -> (FakeTransport, Result<CommitStanding, GithubError>) {
    let transport = FakeTransport::new(responses);
    let repo = RepoRef::parse("serde-rs/serde").expect("repo");
    let found = commit_standing(
        &transport,
        "sekret-xyz",
        &repo,
        &release(),
        SHA,
        &mut RecordingPacer::default(),
    );
    (transport, found)
}

fn on_master() -> CommitStanding {
    CommitStanding::Held {
        held_by: "refs/heads/master".to_owned(),
    }
}

#[test]
fn the_lookup_checks_the_commit_then_compares_it_with_the_named_refs() {
    let (transport, found) = lookup(vec![known(), on_default("BEHIND")]);
    assert_eq!(found.expect("looks up"), on_master());
    assert_eq!(transport.urls(), [GRAPHQL, GRAPHQL]);
}

#[test]
fn every_request_sends_the_token_and_the_json_type() {
    let (transport, _) = lookup(vec![known(), on_default("BEHIND")]);
    let expected = vec![
        ("Authorization".to_owned(), "Bearer sekret-xyz".to_owned()),
        ("Content-Type".to_owned(), "application/json".to_owned()),
    ];
    assert_eq!(transport.requests(), [expected.clone(), expected]);
}

#[test]
fn a_401_on_any_request_is_an_error_that_names_the_token() {
    for responses in [
        vec![FakeResponse::status(401)],
        vec![known(), FakeResponse::status(401)],
    ] {
        let (_, found) = lookup(responses);
        let error = found.expect_err("rejected");
        assert!(
            matches!(error, GithubError::Status { status: 401, .. }),
            "{error}"
        );
        assert!(error.to_string().contains("GITHUB_TOKEN"), "{error}");
        assert!(!error.to_string().contains("sekret-xyz"));
    }
}

#[test]
fn other_statuses_are_errors() {
    let (_, found) = lookup(vec![known(), FakeResponse::status(500)]);
    assert!(matches!(
        found,
        Err(GithubError::Status { status: 500, .. })
    ));
}

#[test]
fn a_body_that_is_not_json_is_an_error() {
    let (_, found) = lookup(vec![FakeResponse::with_body(200, &[], "not json")]);
    assert!(matches!(found, Err(GithubError::Json { .. })));
    let (_, found) = lookup(vec![known(), FakeResponse::with_body(200, &[], "not json")]);
    assert!(matches!(found, Err(GithubError::Json { .. })));
}

#[test]
fn a_rate_limit_answer_is_retried_after_a_wait() {
    let limited = FakeResponse::with_body(429, &[("Retry-After", "3")], "");
    let transport = FakeTransport::new([limited, known(), on_default("IDENTICAL")]);
    let repo = RepoRef::parse("serde-rs/serde").expect("repo");
    let mut pacer = RecordingPacer::default();
    let found =
        commit_standing(&transport, "t", &repo, &release(), SHA, &mut pacer).expect("looks up");
    assert_eq!(found, on_master());
    assert_eq!(pacer.waits.first(), Some(&Some("3".to_owned())));
    assert_eq!(pacer.waits.len(), 2);
}
