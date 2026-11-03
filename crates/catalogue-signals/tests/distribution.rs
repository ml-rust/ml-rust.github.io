#[path = "common/distribution_responses.rs"]
mod distribution_responses;
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

use catalogue_core::Date;
use catalogue_signals::distribution::{
    DistributionError, DockerPulls, HomebrewInstalls, RepologyPackaging, docker_pulls,
    homebrew_installs, release_downloads, repology_packaging,
};
use catalogue_signals::github::RepoRef;
use distribution_responses::{
    NEW_REPO, NGINX_REPO, NO_ANALYTICS_FORMULA, NULL_UPDATED_REPO, RIPGREP_FORMULA,
    RIPGREP_PACKAGES, RIPGREP_RELEASES,
};
use recording_pacer::RecordingPacer;
use transport::{FakeResponse, FakeTransport};

const RELEASES_URL: &str = "https://api.github.com/repos/BurntSushi/ripgrep/releases?per_page=3";
const HOMEBREW_URL: &str = "https://formulae.brew.sh/api/formula/ripgrep.json";
const DOCKER_URL: &str = "https://hub.docker.com/v2/repositories/library/nginx/";
const REPOLOGY_URL: &str = "https://repology.org/api/v1/project/ripgrep";

fn ok(body: &str) -> FakeResponse {
    FakeResponse::with_body(200, &[], body)
}

fn day(year: i16, month: i8, day: i8) -> Option<Date> {
    Some(Date::new(year, month, day).expect("date"))
}

fn ripgrep() -> RepoRef {
    RepoRef::parse("BurntSushi/ripgrep").expect("repo")
}

fn releases(
    transport: &FakeTransport,
) -> Result<Option<catalogue_signals::distribution::ReleaseDownloads>, DistributionError> {
    release_downloads(
        transport,
        "ghp_secret",
        &ripgrep(),
        &mut RecordingPacer::default(),
    )
}

#[test]
fn releases_sum_binary_assets_of_the_first_three_non_draft_releases() {
    let transport = FakeTransport::new([ok(RIPGREP_RELEASES)]);
    let found = releases(&transport).expect("lookup").expect("found");
    let summary: Vec<_> = found
        .releases
        .iter()
        .map(|r| {
            (
                r.tag.as_str(),
                r.published,
                r.prerelease,
                r.binary_downloads,
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("15.2.0", day(2026, 7, 15), false, 5_580_009),
            ("15.1.0", day(2025, 10, 22), false, 14_148_551),
            ("15.0.0-beta", day(2025, 10, 16), true, 473_168),
        ]
    );
    assert_eq!(found.total_binary_downloads, 20_201_728);
}

#[test]
fn releases_send_the_github_headers_and_the_token() {
    let transport = FakeTransport::new([ok("[]")]);
    releases(&transport).expect("lookup");
    assert_eq!(transport.urls(), [RELEASES_URL]);
    let headers = &transport.requests()[0];
    for (name, value) in [
        ("Accept", "application/vnd.github+json"),
        ("X-GitHub-Api-Version", "2022-11-28"),
        ("Authorization", "Bearer ghp_secret"),
    ] {
        assert!(
            headers.iter().any(|(n, v)| n == name && v == value),
            "{name}"
        );
    }
}

#[test]
fn a_repo_with_no_releases_gives_zero() {
    let transport = FakeTransport::new([ok("[]")]);
    let found = releases(&transport).expect("lookup").expect("found");
    assert!(found.releases.is_empty());
    assert_eq!(found.total_binary_downloads, 0);
}

#[test]
fn a_missing_repo_gives_none_for_releases() {
    let transport = FakeTransport::new([FakeResponse::with_body(
        404,
        &[],
        r#"{"message":"Not Found"}"#,
    )]);
    assert_eq!(releases(&transport).expect("lookup"), None);
}

#[test]
fn a_rejected_token_names_the_token_variable() {
    let transport = FakeTransport::new([FakeResponse::with_body(401, &[], "{}")]);
    let error = releases(&transport).expect_err("unauthorized");
    let text = error.to_string();
    assert!(text.starts_with("GitHub Releases: "), "{text}");
    assert!(
        text.contains(RELEASES_URL) && text.contains("401"),
        "{text}"
    );
    assert!(
        text.ends_with("set GITHUB_TOKEN to a valid token"),
        "{text}"
    );
    assert!(!text.contains("ghp_secret"), "{text}");
}

#[test]
fn homebrew_sums_every_key_of_every_window() {
    let transport = FakeTransport::new([ok(RIPGREP_FORMULA)]);
    let found = homebrew_installs(&transport, "ripgrep", &mut RecordingPacer::default())
        .expect("lookup")
        .expect("found");
    assert_eq!(
        found,
        HomebrewInstalls {
            install_30d: 47_114,
            install_90d: 149_200,
            install_365d: 703_181,
            on_request_30d: 36_262,
            on_request_90d: 114_494,
            on_request_365d: 505_036,
        }
    );
    assert_eq!(transport.urls(), [HOMEBREW_URL]);
}

#[test]
fn homebrew_without_analytics_gives_zeros_and_a_missing_formula_none() {
    let transport = FakeTransport::new([ok(NO_ANALYTICS_FORMULA), FakeResponse::status(404)]);
    let mut pacer = RecordingPacer::default();
    let zeros = homebrew_installs(&transport, "fresh", &mut pacer)
        .expect("lookup")
        .expect("found");
    assert_eq!((zeros.install_365d, zeros.on_request_30d), (0, 0));
    assert_eq!(
        homebrew_installs(&transport, "nope", &mut pacer).expect("lookup"),
        None
    );
}

#[test]
fn homebrew_encodes_the_formula_name() {
    let transport = FakeTransport::new([FakeResponse::status(404)]);
    let _ = homebrew_installs(&transport, "python@3.12", &mut RecordingPacer::default());
    assert_eq!(
        transport.urls(),
        ["https://formulae.brew.sh/api/formula/python%403.12.json"]
    );
}

#[test]
fn docker_gives_the_pull_count_and_the_update_time() {
    let transport = FakeTransport::new([ok(NGINX_REPO)]);
    let found = docker_pulls(
        &transport,
        "library",
        "nginx",
        &mut RecordingPacer::default(),
    )
    .expect("lookup")
    .expect("found");
    assert_eq!(found.pull_count, 13_546_273_780);
    assert_eq!(
        found.last_updated.map(|t| t.to_string()).as_deref(),
        Some("2026-11-03T04:52:35.923567Z")
    );
    assert_eq!(transport.urls(), [DOCKER_URL]);
}

#[test]
fn docker_without_last_updated_gives_none_for_it() {
    for body in [NEW_REPO, NULL_UPDATED_REPO] {
        let transport = FakeTransport::new([ok(body)]);
        let found = docker_pulls(&transport, "acme", "tool", &mut RecordingPacer::default())
            .expect("lookup")
            .expect("found");
        assert_eq!(
            found,
            DockerPulls {
                pull_count: 12,
                last_updated: None
            }
        );
    }
}

#[test]
fn docker_names_a_bad_timestamp_and_a_missing_repo() {
    let transport = FakeTransport::new([
        ok(r#"{"pull_count":1,"last_updated":"soon"}"#),
        FakeResponse::with_body(404, &[], r#"{"message":"object not found"}"#),
    ]);
    let mut pacer = RecordingPacer::default();
    let error = docker_pulls(&transport, "a", "b", &mut pacer).expect_err("bad field");
    assert!(error.to_string().contains("`last_updated`"), "{error}");
    assert_eq!(
        docker_pulls(&transport, "a", "c", &mut pacer).expect("lookup"),
        None
    );
}

#[test]
fn repology_groups_repos_into_families() {
    let transport = FakeTransport::new([ok(RIPGREP_PACKAGES)]);
    let found = repology_packaging(&transport, "ripgrep", &mut RecordingPacer::default())
        .expect("lookup")
        .expect("found");
    assert_eq!(
        found,
        RepologyPackaging {
            repos: [
                "alpine_3_21",
                "alpine_edge",
                "debian_12",
                "debian_unstable",
                "fedora_39",
                "homebrew",
                "ubuntu_24_04",
            ]
            .map(str::to_owned)
            .to_vec(),
            families: 5,
        }
    );
    assert_eq!(transport.urls(), [REPOLOGY_URL]);
}

#[test]
fn repology_treats_an_empty_list_and_a_404_as_unknown() {
    let transport = FakeTransport::new([ok("[]"), FakeResponse::status(404)]);
    let mut pacer = RecordingPacer::default();
    assert_eq!(
        repology_packaging(&transport, "zzz", &mut pacer).expect("lookup"),
        None
    );
    assert_eq!(
        repology_packaging(&transport, "zzz", &mut pacer).expect("lookup"),
        None
    );
}

#[test]
fn a_rate_limit_answer_is_retried_after_the_pacer_waits() {
    let transport = FakeTransport::new([
        FakeResponse::with_body(429, &[("retry-after", "7")], "slow down"),
        ok(RIPGREP_PACKAGES),
    ]);
    let mut pacer = RecordingPacer::default();
    let found = repology_packaging(&transport, "ripgrep", &mut pacer)
        .expect("lookup")
        .expect("found");
    assert_eq!(found.families, 5);
    assert_eq!(transport.urls(), [REPOLOGY_URL, REPOLOGY_URL]);
    assert_eq!(pacer.waits, [Some("7".to_owned())]);
}

#[test]
fn a_rate_limit_that_stays_is_an_error_naming_the_api() {
    let limited = || FakeResponse::with_body(429, &[("retry-after", "1")], "");
    let transport = FakeTransport::new([limited(), limited(), limited(), limited()]);
    let error = docker_pulls(
        &transport,
        "library",
        "nginx",
        &mut RecordingPacer::default(),
    )
    .expect_err("rate limited");
    assert!(
        matches!(
            error,
            DistributionError::RetriesExhausted {
                api: "Docker Hub",
                retries: 3,
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn bad_json_names_the_api_and_the_url() {
    for (api, url) in [("Homebrew", HOMEBREW_URL), ("Repology", REPOLOGY_URL)] {
        let transport = FakeTransport::new([ok("<html>")]);
        let mut pacer = RecordingPacer::default();
        let result = if api == "Homebrew" {
            homebrew_installs(&transport, "ripgrep", &mut pacer).map(|_| ())
        } else {
            repology_packaging(&transport, "ripgrep", &mut pacer).map(|_| ())
        };
        let error = result.expect_err("bad json");
        assert!(matches!(error, DistributionError::Json { .. }), "{error}");
        let text = error.to_string();
        assert!(
            text.starts_with(&format!("{api}: ")) && text.contains(url),
            "{text}"
        );
    }
}

#[test]
fn a_server_error_names_the_api_the_url_and_a_next_action() {
    let transport = FakeTransport::new([FakeResponse::status(500)]);
    let error = releases(&transport).expect_err("server error");
    assert_eq!(
        error.to_string(),
        format!("GitHub Releases: {RELEASES_URL} answered with status 500. rerun later")
    );
}
