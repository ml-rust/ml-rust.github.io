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
use catalogue_signals::link::{CommitStanding, commit_standing};
use recording_pacer::RecordingPacer;
use transport::{FakeResponse, FakeTransport};

const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";

fn repo_json(branch: &str) -> FakeResponse {
    FakeResponse::with_body(
        200,
        &[],
        &format!(r#"{{"id":1,"default_branch":"{branch}"}}"#),
    )
}

fn compare(status: &str) -> FakeResponse {
    FakeResponse::with_body(
        200,
        &[],
        &format!(r#"{{"status":"{status}","ahead_by":3}}"#),
    )
}

fn lookup(responses: Vec<FakeResponse>) -> (FakeTransport, Result<CommitStanding, GithubError>) {
    let transport = FakeTransport::new(responses);
    let repo = RepoRef::parse("serde-rs/serde").expect("repo");
    let found = commit_standing(
        &transport,
        "sekret-xyz",
        &repo,
        SHA,
        &mut RecordingPacer::default(),
    );
    (transport, found)
}

fn standing(status: &str) -> CommitStanding {
    let (_, found) = lookup(vec![repo_json("master"), compare(status)]);
    found.expect("looks up")
}

#[test]
fn the_lookup_reads_the_default_branch_then_compares_the_commit_with_it() {
    let (transport, found) = lookup(vec![repo_json("master"), compare("ahead")]);
    assert_eq!(found.expect("looks up"), CommitStanding::OnDefaultBranch);
    assert_eq!(
        transport.urls(),
        [
            "https://api.github.com/repos/serde-rs/serde".to_owned(),
            format!(
                "https://api.github.com/repos/serde-rs/serde/compare/{SHA}...master?per_page=1"
            ),
        ]
    );
}

#[test]
fn both_requests_send_the_json_media_type_and_the_token() {
    let (transport, _) = lookup(vec![repo_json("master"), compare("ahead")]);
    let expected = vec![
        (
            "Accept".to_owned(),
            "application/vnd.github+json".to_owned(),
        ),
        ("X-GitHub-Api-Version".to_owned(), "2022-11-28".to_owned()),
        ("Authorization".to_owned(), "Bearer sekret-xyz".to_owned()),
    ];
    assert_eq!(transport.requests(), [expected.clone(), expected]);
}

#[test]
fn ahead_and_identical_mean_the_default_branch_contains_the_commit() {
    assert_eq!(standing("ahead"), CommitStanding::OnDefaultBranch);
    assert_eq!(standing("identical"), CommitStanding::OnDefaultBranch);
}

#[test]
fn behind_and_diverged_mean_the_default_branch_lacks_the_commit() {
    assert_eq!(standing("behind"), CommitStanding::NotOnDefaultBranch);
    assert_eq!(standing("diverged"), CommitStanding::NotOnDefaultBranch);
}

#[test]
fn an_unknown_status_does_not_verify() {
    assert_eq!(standing("surprise"), CommitStanding::NotOnDefaultBranch);
}

#[test]
fn a_404_or_422_on_the_compare_means_the_commit_is_missing() {
    for status in [404, 422] {
        let (_, found) = lookup(vec![repo_json("master"), FakeResponse::status(status)]);
        assert_eq!(
            found.expect("looks up"),
            CommitStanding::Missing,
            "{status}"
        );
    }
}

#[test]
fn a_404_on_the_repo_means_missing_and_sends_no_compare() {
    let (transport, found) = lookup(vec![FakeResponse::status(404)]);
    assert_eq!(found.expect("looks up"), CommitStanding::Missing);
    assert_eq!(transport.urls().len(), 1);
}

#[test]
fn a_branch_name_is_encoded_per_segment() {
    let (transport, _) = lookup(vec![repo_json("release/1.x v2"), compare("ahead")]);
    assert_eq!(
        transport.urls()[1],
        format!(
            "https://api.github.com/repos/serde-rs/serde/compare/{SHA}...release/1.x%20v2?per_page=1"
        )
    );
}

#[test]
fn a_401_on_either_request_is_an_error_that_names_the_token() {
    for responses in [
        vec![FakeResponse::status(401)],
        vec![repo_json("master"), FakeResponse::status(401)],
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
    let (_, found) = lookup(vec![repo_json("master"), FakeResponse::status(500)]);
    assert!(matches!(
        found,
        Err(GithubError::Status { status: 500, .. })
    ));
}

#[test]
fn a_body_that_is_not_the_expected_json_is_an_error() {
    let (_, found) = lookup(vec![FakeResponse::with_body(200, &[], "{}")]);
    assert!(matches!(found, Err(GithubError::Json { .. })));
    let (_, found) = lookup(vec![
        repo_json("master"),
        FakeResponse::with_body(200, &[], "{}"),
    ]);
    assert!(matches!(found, Err(GithubError::Json { .. })));
}

#[test]
fn a_rate_limit_answer_is_retried_after_a_wait() {
    let limited = FakeResponse::with_body(429, &[("Retry-After", "3")], "");
    let transport = FakeTransport::new([limited, repo_json("main"), compare("identical")]);
    let repo = RepoRef::parse("serde-rs/serde").expect("repo");
    let mut pacer = RecordingPacer::default();
    let found = commit_standing(&transport, "t", &repo, SHA, &mut pacer).expect("looks up");
    assert_eq!(found, CommitStanding::OnDefaultBranch);
    assert_eq!(pacer.waits.first(), Some(&Some("3".to_owned())));
    assert_eq!(pacer.waits.len(), 2);
}
