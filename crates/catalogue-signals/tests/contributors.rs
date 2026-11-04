//! Counting contributors from the `Link` header of one REST request.

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

use catalogue_signals::fallback::{Failure, github_failure};
use catalogue_signals::github::{Contributors, GithubError, RepoRef, repo_contributors};
use recording_pacer::RecordingPacer;
use transport::{FakeResponse, FakeTransport};

const URL: &str = "https://api.github.com/repos/ml-rust/numr/contributors?per_page=1&anon=false";
const TOKEN: &str = "ghp_secret";
const TOO_LARGE: &str = r#"{"message":"The history or contributor list is too large to list contributors for this repository via the API.","documentation_url":"https://docs.github.com/rest"}"#;

fn numr() -> RepoRef {
    RepoRef::parse("ml-rust/numr").expect("repo")
}

fn count(transport: &FakeTransport) -> Result<Contributors, GithubError> {
    repo_contributors(transport, TOKEN, &numr(), &mut RecordingPacer::default())
}

#[test]
fn the_last_page_of_the_link_header_is_the_count() {
    let link = "<https://api.github.com/repositories/1/contributors?per_page=1&anon=false&page=2>; rel=\"next\", \
                <https://api.github.com/repositories/1/contributors?per_page=1&anon=false&page=38>; rel=\"last\"";
    let transport = FakeTransport::new([FakeResponse::with_body(
        200,
        &[("Link", link)],
        r#"[{"login":"a"}]"#,
    )]);
    assert_eq!(
        count(&transport).expect("counts"),
        Contributors::Counted { count: 38 }
    );
    assert_eq!(transport.urls(), [URL], "one request");
}

#[test]
fn the_request_carries_only_the_bearer_token_and_the_rest_headers() {
    let transport = FakeTransport::new([FakeResponse::with_body(200, &[], "[]")]);
    count(&transport).expect("counts");
    let requests = transport.requests();
    let headers = requests.first().expect("one request");
    assert!(
        headers.contains(&("Authorization".to_owned(), format!("Bearer {TOKEN}"))),
        "{headers:?}"
    );
    let names: Vec<&str> = headers.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["Accept", "X-GitHub-Api-Version", "Authorization"]);
}

#[test]
fn a_single_page_with_no_link_header_counts_its_body() {
    let one = FakeTransport::new([FakeResponse::with_body(200, &[], r#"[{"login":"a"}]"#)]);
    assert_eq!(
        count(&one).expect("counts"),
        Contributors::Counted { count: 1 }
    );
    let none = FakeTransport::new([FakeResponse::with_body(200, &[], "[]")]);
    assert_eq!(
        count(&none).expect("counts"),
        Contributors::Counted { count: 0 }
    );
}

#[test]
fn a_link_header_with_no_last_link_counts_the_body() {
    let link = "<https://api.github.com/repositories/1/contributors?page=1>; rel=\"first\"";
    let transport = FakeTransport::new([FakeResponse::with_body(
        200,
        &[("Link", link)],
        r#"[{"login":"a"}]"#,
    )]);
    assert_eq!(
        count(&transport).expect("counts"),
        Contributors::Counted { count: 1 }
    );
}

#[test]
fn an_empty_repo_counts_zero() {
    let transport = FakeTransport::new([FakeResponse::status(204)]);
    assert_eq!(
        count(&transport).expect("counts"),
        Contributors::Counted { count: 0 }
    );
}

#[test]
fn a_403_for_a_list_too_large_is_an_answer_and_no_error() {
    let transport = FakeTransport::new([FakeResponse::with_body(403, &[], TOO_LARGE)]);
    assert_eq!(count(&transport).expect("answers"), Contributors::TooLarge);
    assert_eq!(transport.urls().len(), 1, "no retry");
}

#[test]
fn any_other_403_falls_back_and_never_prints_the_token() {
    let transport = FakeTransport::new([FakeResponse::with_body(
        403,
        &[],
        r#"{"message":"Resource not accessible by integration"}"#,
    )]);
    let error = count(&transport).expect_err("a refusal");
    assert!(matches!(error, GithubError::Status { status: 403, .. }));
    assert_eq!(github_failure(&error), Failure::Fallback);
    let message = error.to_string();
    assert!(message.contains(URL), "{message}");
    assert!(!message.contains(TOKEN), "{message}");
}

#[test]
fn a_rate_limited_403_is_retried_even_when_its_body_says_too_large() {
    let transport = FakeTransport::new([
        FakeResponse::with_body(403, &[("Retry-After", "1")], TOO_LARGE),
        FakeResponse::with_body(200, &[], r#"[{"login":"a"}]"#),
    ]);
    let mut pacer = RecordingPacer::default();
    let found = repo_contributors(&transport, TOKEN, &numr(), &mut pacer).expect("counts");
    assert_eq!(found, Contributors::Counted { count: 1 });
    assert_eq!(pacer.waits, [Some("1".to_owned())]);
}

#[test]
fn a_failed_fetch_is_an_error_the_fallback_rules_sort() {
    let down = FakeTransport::new([FakeResponse::status(500)]);
    assert_eq!(
        github_failure(&count(&down).expect_err("500")),
        Failure::Fallback
    );
    let gone = FakeTransport::new([FakeResponse::status(404)]);
    assert_eq!(
        github_failure(&count(&gone).expect_err("404")),
        Failure::Definitive
    );
    let rejected = FakeTransport::new([FakeResponse::status(401)]);
    assert_eq!(
        github_failure(&count(&rejected).expect_err("401")),
        Failure::Fatal
    );
    let offline = FakeTransport::new([]);
    assert_eq!(
        github_failure(&count(&offline).expect_err("no response")),
        Failure::Fallback
    );
}

#[test]
fn a_link_with_no_page_number_or_a_body_that_is_no_list_falls_back() {
    let link = "<https://api.github.com/repositories/1/contributors>; rel=\"last\"";
    let transport = FakeTransport::new([FakeResponse::with_body(200, &[("Link", link)], "[]")]);
    let error = count(&transport).expect_err("no page");
    assert!(matches!(error, GithubError::BadLink { .. }), "{error}");
    assert!(error.to_string().contains(link), "{error}");
    assert_eq!(github_failure(&error), Failure::Fallback);
    let broken = FakeTransport::new([FakeResponse::with_body(200, &[], "{}")]);
    let error = count(&broken).expect_err("not a list");
    assert!(matches!(error, GithubError::Json { .. }), "{error}");
    assert_eq!(github_failure(&error), Failure::Fallback);
}
