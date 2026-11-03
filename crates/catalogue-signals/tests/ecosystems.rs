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

use catalogue_signals::ecosystems::{CrateDependents, EcosystemsError, crate_dependents};
use catalogue_signals::http::{Headers, Pacer};
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

/// The `candle-core` package fields the lookup reads, as recorded from the API.
const CANDLE_CORE: &str = r#"{"id":8020688,"name":"candle-core","ecosystem":"cargo",
    "repository_url":"https://github.com/huggingface/candle","dependent_packages_count":47,
    "downloads":10982240,"dependent_repos_count":1,
    "rankings":{"downloads":12.4,"dependent_repos_count":16.4,"average":6.5},
    "docker_dependents_count":9,"docker_downloads_count":18669296,"status":null}"#;

const URL: &str = "https://packages.ecosyste.ms/api/v1/registries/crates.io/packages/candle-core";

fn limits() -> [(&'static str, &'static str); 2] {
    [
        ("x-ratelimit-limit", "5000"),
        ("x-ratelimit-remaining", "4999"),
    ]
}

#[test]
fn a_known_crate_gives_its_counts() {
    let transport = FakeTransport::new([FakeResponse::with_body(200, &limits(), CANDLE_CORE)]);
    let found = crate_dependents(&transport, "candle-core", &mut RecordingPacer::default())
        .expect("lookup")
        .expect("found");
    assert_eq!(
        found,
        CrateDependents {
            dependent_packages: 47,
            dependent_repos: 1,
            docker_dependents: 9,
            docker_downloads: 18_669_296,
            fields_missing: Vec::new(),
        }
    );
    assert_eq!(transport.urls(), [URL]);
    assert!(
        transport.requests()[0]
            .iter()
            .any(|(name, value)| name == "Accept" && value == "application/json")
    );
}

#[test]
fn an_unknown_crate_gives_none() {
    let transport = FakeTransport::new([FakeResponse::with_body(
        404,
        &limits(),
        r#"{"error":"not found"}"#,
    )]);
    let found = crate_dependents(
        &transport,
        "zzz-no-such-crate",
        &mut RecordingPacer::default(),
    );
    assert_eq!(found.expect("lookup"), None);
}

#[test]
fn null_and_absent_fields_are_zero_and_listed() {
    let body = r#"{"name":"x","dependent_packages_count":null,"dependent_repos_count":3,
        "docker_downloads_count":null}"#;
    let transport = FakeTransport::new([FakeResponse::with_body(200, &[], body)]);
    let found = crate_dependents(&transport, "x", &mut RecordingPacer::default())
        .expect("lookup")
        .expect("found");
    assert_eq!(found.dependent_packages, 0);
    assert_eq!(found.dependent_repos, 3);
    assert_eq!(found.docker_dependents, 0);
    assert_eq!(found.docker_downloads, 0);
    assert_eq!(
        found.fields_missing,
        [
            "dependent_packages_count",
            "docker_dependents_count",
            "docker_downloads_count"
        ]
    );
}

#[test]
fn a_rate_limit_answer_is_retried_after_the_pacer_waits() {
    let transport = FakeTransport::new([
        FakeResponse::with_body(429, &[("retry-after", "7")], r#"{"error":"slow down"}"#),
        FakeResponse::with_body(200, &[], CANDLE_CORE),
    ]);
    let mut pacer = RecordingPacer::default();
    let found = crate_dependents(&transport, "candle-core", &mut pacer)
        .expect("lookup")
        .expect("found");
    assert_eq!(found.dependent_packages, 47);
    assert_eq!(transport.urls(), [URL, URL]);
    assert_eq!(pacer.waits, [Some("7".to_owned())]);
}

#[test]
fn a_rate_limit_that_stays_is_an_error() {
    let limited = || FakeResponse::with_body(429, &[("retry-after", "1")], "");
    let transport = FakeTransport::new([limited(), limited(), limited(), limited()]);
    let error = crate_dependents(&transport, "candle-core", &mut RecordingPacer::default())
        .expect_err("rate limited");
    assert!(
        matches!(error, EcosystemsError::RetriesExhausted { retries: 3, .. }),
        "{error}"
    );
    assert!(error.to_string().contains(URL), "{error}");
}

#[test]
fn bad_json_names_the_url() {
    let transport = FakeTransport::new([FakeResponse::with_body(200, &[], "<html>")]);
    let error = crate_dependents(&transport, "candle-core", &mut RecordingPacer::default())
        .expect_err("bad json");
    assert!(matches!(error, EcosystemsError::Json { .. }), "{error}");
    assert!(error.to_string().contains(URL), "{error}");
}

#[test]
fn a_server_error_names_the_url_and_a_next_action() {
    let transport = FakeTransport::new([FakeResponse::status(500)]);
    let error = crate_dependents(&transport, "candle-core", &mut RecordingPacer::default())
        .expect_err("server error");
    assert_eq!(
        error.to_string(),
        format!("{URL} answered with status 500. rerun later")
    );
}

#[test]
fn the_name_is_encoded_as_a_path_segment() {
    let transport = FakeTransport::new([FakeResponse::with_body(404, &[], "{}")]);
    let _ = crate_dependents(&transport, "a/b c", &mut RecordingPacer::default());
    assert_eq!(
        transport.urls(),
        ["https://packages.ecosyste.ms/api/v1/registries/crates.io/packages/a%2Fb%20c"]
    );
}
