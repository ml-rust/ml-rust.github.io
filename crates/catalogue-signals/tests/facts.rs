#[path = "common/facts_responses.rs"]
mod facts_responses;
#[path = "common/facts_setup.rs"]
mod facts_setup;
#[path = "common/post_bodies.rs"]
mod post_bodies;
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

use catalogue_signals::github::{CiState, GithubError, RepoFacts, repo_facts};
use facts_responses::{RepoBody, commit, item};
use facts_setup::{TODAY, TOKEN, found, read, requested};
use jiff::civil::date;
use recording_pacer::RecordingPacer;
use serde_json::Value;
use transport::{FakeResponse, FakeTransport};

/// A 200 response for a repo that does not exist.
fn gone() -> FakeResponse {
    FakeResponse::with_body(
        200,
        &[],
        r#"{"data":{"repository":null},"errors":[{"type":"NOT_FOUND","path":["repository"],"locations":[{"line":1,"column":9}],"message":"Could not resolve to a Repository with the name 'a/b'."}]}"#,
    )
}

#[test]
fn a_found_repo_reports_its_state() {
    let mut body = RepoBody::new("a/b");
    body.total_commits = 2;
    body.commits = vec![
        commit("2026-10-26T17:14:40Z", Some("alice"), "Alice"),
        commit("2026-09-01T00:00:00Z", Some("bob"), "Bob"),
    ];
    body.issues = vec![item("2026-10-27T11:53:29Z", "OPEN", "NONE", "u", vec![])];
    body.pulls = vec![item("2026-10-27T10:53:22Z", "CLOSED", "NONE", "u", vec![])];
    body.latest_total = 4321;
    let facts = found(&body);
    assert_eq!(facts.commits, Some(4321));
    assert_eq!(facts.name_with_owner, "a/b");
    assert!(!facts.moved && !facts.archived && facts.fork);
    assert_eq!(facts.stars, 64548);
    assert_eq!(facts.forks, Some(8123));
    assert_eq!(
        facts.pushed_at.map(|t| t.to_string()).as_deref(),
        Some("2026-10-26T17:14:42Z")
    );
    assert_eq!(facts.ci, Some(CiState::Success));
    assert_eq!(facts.last_human_commit, Some(date(2026, 10, 26)));
    assert_eq!(facts.human_commits_365, 2);
    assert!(!facts.human_commits_capped);
    assert!(!facts.last_human_commit_beyond_sample);
    assert_eq!(facts.items_sampled, 2);
    assert_eq!(facts.oldest_unanswered_days, Some(7));
}

#[test]
fn the_request_is_one_authorised_json_post() {
    let transport = FakeTransport::new([RepoBody::new("a/b").response()]);
    read(&transport).expect("reads");
    assert_eq!(transport.urls(), ["https://api.github.com/graphql"]);
    assert_eq!(
        transport.requests()[0],
        [
            ("Authorization".to_owned(), "Bearer sekret-xyz".to_owned()),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ]
    );
    let sent: Value = serde_json::from_slice(&transport.bodies()[0]).expect("json body");
    assert_eq!(sent["variables"]["owner"], "a");
    assert_eq!(sent["variables"]["name"], "b");
    assert_eq!(sent["variables"]["since"], "2025-11-03T00:00:00Z");
    let query = sent["query"].as_str().expect("query text");
    assert!(query.contains("recent: history(first: 100, since: $since)"));
    assert!(query.contains("latest: history(first: 30)"));
}

#[test]
fn a_missing_repo_is_gone() {
    let transport = FakeTransport::new([gone()]);
    let result = read(&transport).expect("reads");
    assert_eq!(
        result,
        RepoFacts::Gone {
            requested: requested()
        }
    );
}

#[test]
fn a_renamed_repo_is_moved() {
    let facts = found(&RepoBody::new("New-Owner/b"));
    assert!(facts.moved);
    assert_eq!(facts.name_with_owner, "New-Owner/b");
    assert!(!found(&RepoBody::new("A/B")).moved);
}

#[test]
fn check_states_map_to_the_enum() {
    let cases = [
        (Some("SUCCESS"), Some(CiState::Success)),
        (Some("FAILURE"), Some(CiState::Failure)),
        (Some("PENDING"), Some(CiState::Pending)),
        (Some("ERROR"), Some(CiState::Error)),
        (Some("EXPECTED"), Some(CiState::Expected)),
        (None, None),
    ];
    for (state, expected) in cases {
        let mut body = RepoBody::new("a/b");
        body.ci = state;
        assert_eq!(found(&body).ci, expected, "{state:?}");
    }
}

#[test]
fn a_graphql_error_names_the_repo_and_the_next_action() {
    let transport = FakeTransport::new([FakeResponse::with_body(
        200,
        &[],
        r#"{"data":null,"errors":[{"type":"FORBIDDEN","message":"Resource not accessible"}]}"#,
    )]);
    let error = read(&transport).expect_err("graphql");
    assert!(matches!(error, GithubError::Graphql { .. }), "{error}");
    let text = error.to_string();
    assert!(
        text.contains("a/b") && text.contains("Resource not accessible"),
        "{text}"
    );
    assert!(text.contains("then rerun"), "{text}");
    assert!(!text.contains("sekret"), "{text}");
}

#[test]
fn unauthorized_names_the_token() {
    let transport = FakeTransport::new([FakeResponse::status(401)]);
    let error = read(&transport).expect_err("401");
    assert!(
        matches!(error, GithubError::Status { status: 401, .. }),
        "{error}"
    );
    let text = error.to_string();
    assert!(text.contains("set GITHUB_TOKEN to a valid token"), "{text}");
    assert!(!text.contains("sekret"), "{text}");
}

#[test]
fn a_rate_limit_answer_waits_and_retries() {
    let transport = FakeTransport::new([
        FakeResponse::with_body(403, &[("Retry-After", "7")], "{}"),
        RepoBody::new("a/b").response(),
    ]);
    let mut pacer = RecordingPacer::default();
    let result = repo_facts(&transport, TOKEN, &requested(), TODAY, &mut pacer).expect("retries");
    assert!(matches!(result, RepoFacts::Found(_)));
    assert_eq!(pacer.waits, [Some("7".to_owned())]);
    assert_eq!(transport.bodies().len(), 2);
}

#[test]
fn bad_json_names_the_endpoint() {
    let transport = FakeTransport::new([FakeResponse::with_body(200, &[], "not json")]);
    let error = read(&transport).expect_err("json");
    assert!(matches!(error, GithubError::Json { .. }));
    assert!(error.to_string().contains("https://api.github.com/graphql"));
}
