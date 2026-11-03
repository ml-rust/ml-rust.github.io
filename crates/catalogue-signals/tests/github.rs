#[path = "common/search_responses.rs"]
mod search_responses;
#[path = "common/transport.rs"]
mod transport;

use catalogue_signals::github::{GithubError, GithubRules, Pacer, search_repos_on};
use catalogue_signals::http::Headers;
use jiff::civil::{Date, date};
use transport::{FakeResponse, FakeTransport};

/// A pacer that records the `retry-after` of each response it waits on.
#[derive(Default)]
struct RecordingPacer {
    waits: Vec<Option<String>>,
}

impl Pacer for RecordingPacer {
    fn wait(&mut self, response_headers: &Headers) {
        self.waits
            .push(response_headers.get("retry-after").map(str::to_owned));
    }
}

const TODAY: Date = date(2008, 1, 3);
const TOKEN: &str = "sekret-xyz";

fn rules(topics: &[&str]) -> GithubRules {
    GithubRules {
        topics: topics.iter().map(|t| (*t).to_owned()).collect(),
        min_stars: 10,
    }
}

fn item(name: &str, stars: u32) -> String {
    format!(
        r#"{{"full_name":"{name}","html_url":"https://github.com/{name}","description":"d",
        "stargazers_count":{stars},"pushed_at":"2026-01-02T03:04:05Z",
        "created_at":"2020-01-01T00:00:00Z","topics":["llm"],"archived":false,"fork":false,
        "homepage":null,"id":9}}"#
    )
}

fn page(total: u64, names: &[String]) -> FakeResponse {
    let items: Vec<String> = names.iter().map(|n| item(n, 50)).collect();
    FakeResponse::with_body(
        200,
        &[],
        &format!(r#"{{"total_count":{total},"items":[{}]}}"#, items.join(",")),
    )
}

fn names(prefix: &str, count: usize) -> Vec<String> {
    (0..count).map(|i| format!("{prefix}/r{i}")).collect()
}

fn run(
    transport: &FakeTransport,
    topics: &[&str],
    pacer: &mut RecordingPacer,
) -> Result<catalogue_signals::github::GithubFound, GithubError> {
    search_repos_on(transport, TOKEN, &rules(topics), pacer, TODAY)
}

fn created_range(url: &str) -> String {
    let encoded = url.split("created%3A").nth(1).expect("created clause");
    encoded.split('&').next().expect("range").to_owned()
}

#[test]
fn a_single_page_returns_its_repos() {
    let transport = FakeTransport::new([page(2, &["a/x".to_owned(), "b/y".to_owned()])]);
    let mut pacer = RecordingPacer::default();
    let found = run(&transport, &["llm"], &mut pacer).expect("searches");
    assert_eq!(found.requests, 1);
    assert!(found.warnings.is_empty());
    let listed: Vec<&str> = found
        .repos
        .iter()
        .map(|f| f.repo.full_name.as_str())
        .collect();
    assert_eq!(listed, ["a/x", "b/y"]);
    assert_eq!(found.repos[0].matched_topics, ["llm"]);
    assert_eq!(found.repos[0].repo.stargazers_count, 50);
    assert!(pacer.waits.is_empty());
}

#[test]
fn requests_send_the_api_headers_and_the_token() {
    let transport = FakeTransport::new([page(0, &[])]);
    run(&transport, &["llm"], &mut RecordingPacer::default()).expect("searches");
    let sent = transport.requests();
    assert_eq!(
        sent[0],
        [
            (
                "Accept".to_owned(),
                "application/vnd.github+json".to_owned()
            ),
            ("X-GitHub-Api-Version".to_owned(), "2022-11-28".to_owned()),
            ("Authorization".to_owned(), "Bearer sekret-xyz".to_owned()),
        ]
    );
}

#[test]
fn pages_are_read_until_every_item_is_in() {
    let first = names("a", 100);
    let second = names("b", 100);
    let third = names("c", 20);
    let transport = FakeTransport::new([page(220, &first), page(220, &second), page(220, &third)]);
    let mut pacer = RecordingPacer::default();
    let found = run(&transport, &["llm"], &mut pacer).expect("searches");
    assert_eq!(found.repos.len(), 220);
    assert_eq!(found.requests, 3);
    let urls = transport.urls();
    assert!(urls[0].ends_with("&page=1") && urls[1].ends_with("&page=2"));
    assert!(urls[2].ends_with("&page=3"));
    assert_eq!(pacer.waits.len(), 2);
}

#[test]
fn a_range_over_the_cap_is_split_in_half() {
    let transport = FakeTransport::new([
        page(1500, &[]),
        page(1, &["a/left".to_owned()]),
        page(1, &["b/right".to_owned()]),
    ]);
    let found = run(&transport, &["llm"], &mut RecordingPacer::default()).expect("searches");
    let urls = transport.urls();
    assert_eq!(created_range(&urls[0]), "2008-01-01..2008-01-03");
    assert_eq!(created_range(&urls[1]), "2008-01-01..2008-01-02");
    assert_eq!(created_range(&urls[2]), "2008-01-03..2008-01-03");
    assert_eq!(found.repos.len(), 2);
    assert!(found.warnings.is_empty());
}

#[test]
fn a_single_day_over_the_cap_keeps_the_first_thousand_and_warns() {
    let responses: Vec<FakeResponse> = (0..10)
        .map(|chunk| page(1500, &names(&format!("d{chunk}"), 100)))
        .collect();
    let transport = FakeTransport::new(responses);
    let day = date(2008, 1, 1);
    let found = search_repos_on(
        &transport,
        TOKEN,
        &rules(&["llm"]),
        &mut RecordingPacer::default(),
        day,
    )
    .expect("searches");
    assert_eq!(found.repos.len(), 1000);
    assert_eq!(found.requests, 10);
    assert_eq!(found.warnings.len(), 1);
    assert!(found.warnings[0].contains("`llm`"), "{:?}", found.warnings);
    assert!(
        found.warnings[0].contains("2008-01-01"),
        "{:?}",
        found.warnings
    );
    assert!(transport.urls()[9].ends_with("&page=10"));
}

#[test]
fn a_repo_found_by_two_topics_merges_case_insensitively() {
    let transport = FakeTransport::new([
        page(2, &["Owner/Repo".to_owned(), "a/only".to_owned()]),
        page(1, &["owner/repo".to_owned()]),
    ]);
    let found = run(
        &transport,
        &["llm", "inference"],
        &mut RecordingPacer::default(),
    )
    .expect("searches");
    assert_eq!(found.repos.len(), 2);
    let merged = found
        .repos
        .iter()
        .find(|f| f.repo.full_name.eq_ignore_ascii_case("owner/repo"))
        .expect("merged repo");
    assert_eq!(merged.matched_topics, ["llm", "inference"]);
}

#[test]
fn unauthorized_names_the_url_and_the_token() {
    let transport = FakeTransport::new([FakeResponse::status(401)]);
    let error = run(&transport, &["llm"], &mut RecordingPacer::default()).expect_err("401");
    let message = error.to_string();
    assert!(
        message.contains("https://api.github.com/search/repositories?q="),
        "{message}"
    );
    assert!(message.contains("401"), "{message}");
    assert!(
        message.contains("set GITHUB_TOKEN to a valid token"),
        "{message}"
    );
    assert!(!message.contains("sekret"), "{message}");
}

#[test]
fn a_rate_limit_answer_waits_and_retries_the_same_request() {
    let transport = FakeTransport::new([
        FakeResponse::with_body(403, &[("Retry-After", "7")], "{}"),
        page(1, &["a/x".to_owned()]),
    ]);
    let mut pacer = RecordingPacer::default();
    let found = run(&transport, &["llm"], &mut pacer).expect("retries");
    assert_eq!(found.requests, 2);
    assert_eq!(pacer.waits, [Some("7".to_owned())]);
    let urls = transport.urls();
    assert_eq!(urls[0], urls[1]);
}

#[test]
fn retries_stop_after_three() {
    let limited = || FakeResponse::with_body(429, &[("retry-after", "1")], "{}");
    let transport = FakeTransport::new([limited(), limited(), limited(), limited()]);
    let error = run(&transport, &["llm"], &mut RecordingPacer::default()).expect_err("exhausted");
    assert!(
        matches!(error, GithubError::RetriesExhausted { retries: 3, .. }),
        "{error}"
    );
    assert!(error.to_string().contains("https://api.github.com/"));
    assert_eq!(transport.urls().len(), 4);
}

#[test]
fn a_403_without_a_wait_hint_is_a_status_error() {
    let transport = FakeTransport::new([FakeResponse::status(403)]);
    let error = run(&transport, &["llm"], &mut RecordingPacer::default()).expect_err("403");
    assert!(
        matches!(error, GithubError::Status { status: 403, .. }),
        "{error}"
    );
}

#[test]
fn bad_json_names_the_url() {
    let transport = FakeTransport::new([FakeResponse::with_body(200, &[], "not json")]);
    let error = run(&transport, &["llm"], &mut RecordingPacer::default()).expect_err("json");
    assert!(matches!(error, GithubError::Json { .. }));
    assert!(error.to_string().contains("https://api.github.com/"));
}

#[test]
fn the_query_url_is_percent_encoded() {
    let transport = FakeTransport::new([page(0, &[])]);
    run(&transport, &["llm"], &mut RecordingPacer::default()).expect("searches");
    assert_eq!(
        transport.urls()[0],
        "https://api.github.com/search/repositories?q=language%3ARust%20topic%3Allm%20stars%3A%3E%3D10%20archived%3Afalse%20fork%3Afalse%20created%3A2008-01-01..2008-01-03&sort=stars&order=desc&per_page=100&page=1"
    );
}
