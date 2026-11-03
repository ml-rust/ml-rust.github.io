#[path = "common/cache.rs"]
mod cache;
#[path = "common/link_setup.rs"]
mod link_setup;
#[path = "common/package_responses.rs"]
mod package_responses;
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

use cache::cache;
use catalogue_signals::github::{GithubError, RepoRef};
use catalogue_signals::link::{CrateRelease, LinkCheck, LinkError, check_link};
use link_setup::{SHA, compare_code, compared, package, run, then, today, vcs, verified};
use recording_pacer::RecordingPacer;
use transport::{FakeResponse, FakeTransport};

const PACKAGE_URL: &str = "https://static.crates.io/crates/Serde_Json/Serde_Json-1.0.1.crate";

#[test]
fn a_commit_on_the_default_branch_verifies_the_link_and_sends_three_requests() {
    let (_guard, dir) = cache();
    let responses = then(package(Some(&vcs(false))), compared("ahead"));
    let (transport, outcome) = run(responses, "serde-rs/json", &dir);
    let outcome = outcome.expect("checks");
    assert_eq!(outcome.check, verified(false));
    assert!(!outcome.cached);
    let urls = transport.urls();
    assert_eq!(urls[0], PACKAGE_URL);
    assert_eq!(urls[1], "https://api.github.com/repos/serde-rs/json");
    assert_eq!(
        urls[2],
        format!("https://api.github.com/repos/serde-rs/json/compare/{SHA}...main?per_page=1")
    );
    assert_eq!(urls.len(), 3);
    assert!(transport.requests()[0].is_empty());
}

#[test]
fn a_missing_commit_is_recorded_for_404_and_422() {
    for status in [404, 422] {
        let (_guard, dir) = cache();
        let responses = then(package(Some(&vcs(false))), compare_code(status));
        let (_, outcome) = run(responses, "a/b", &dir);
        let expected = LinkCheck::CommitMissing {
            sha1: SHA.to_owned(),
            dirty: false,
        };
        assert_eq!(outcome.expect("checks").check, expected, "{status}");
    }
}

#[test]
fn identical_verifies_the_link() {
    let (_guard, dir) = cache();
    let responses = then(package(Some(&vcs(false))), compared("identical"));
    let (_, outcome) = run(responses, "a/b", &dir);
    assert_eq!(outcome.expect("checks").check, verified(false));
}

#[test]
fn a_commit_the_default_branch_lacks_is_recorded_and_cached() {
    for status in ["behind", "diverged"] {
        let (_guard, dir) = cache();
        let responses = then(package(Some(&vcs(true))), compared(status));
        let (_, outcome) = run(responses, "a/b", &dir);
        let expected = LinkCheck::NotOnDefaultBranch {
            sha1: SHA.to_owned(),
            dirty: true,
        };
        assert_eq!(outcome.expect("checks").check, expected, "{status}");
        let (transport, hit) = run(vec![], "a/b", &dir);
        let hit = hit.expect("hit");
        assert!(hit.cached && hit.check == expected, "{status}");
        assert!(transport.urls().is_empty());
    }
}

#[test]
fn a_dirty_release_is_still_checked() {
    let (_guard, dir) = cache();
    let responses = then(package(Some(&vcs(true))), compared("ahead"));
    let (_, outcome) = run(responses, "a/b", &dir);
    assert_eq!(outcome.expect("checks").check, verified(true));
}

#[test]
fn a_package_without_the_file_or_the_commit_sends_no_github_request() {
    for json in [None, Some(r#"{"path_in_vcs":""}"#)] {
        let (_guard, dir) = cache();
        let (transport, outcome) = run(vec![package(json)], "a/b", &dir);
        assert_eq!(outcome.expect("checks").check, LinkCheck::NoVcsInfo);
        assert_eq!(transport.urls().len(), 1);
    }
}

#[test]
fn a_rejected_token_is_a_github_401() {
    let (_guard, dir) = cache();
    let (_, outcome) = run(
        then(package(Some(&vcs(false))), compare_code(401)),
        "a/b",
        &dir,
    );
    assert!(matches!(
        outcome,
        Err(LinkError::Github(GithubError::Status { status: 401, .. }))
    ));
}

#[test]
fn a_failed_download_names_the_url_and_status() {
    let (_guard, dir) = cache();
    let (_, outcome) = run(vec![FakeResponse::status(404)], "a/b", &dir);
    let error = outcome.expect_err("download fails");
    assert!(
        matches!(error, LinkError::Download { status: 404, .. }),
        "{error}"
    );
    assert!(error.to_string().contains(PACKAGE_URL), "{error}");
}

#[test]
fn a_broken_package_is_an_error() {
    let (_guard, dir) = cache();
    let (_, outcome) = run(
        vec![FakeResponse::bytes(200, b"not gzip".to_vec())],
        "a/b",
        &dir,
    );
    assert!(matches!(outcome, Err(LinkError::Package { .. })));
}

#[test]
fn a_name_that_breaks_a_url_or_a_path_is_rejected_before_any_request() {
    let (_guard, dir) = cache();
    let transport = FakeTransport::new([]);
    let repo = RepoRef::parse("a/b").expect("repo");
    for (name, version) in [("../x", "1.0.0"), ("x", "1/0"), ("", "1.0.0"), ("x", "")] {
        let release = CrateRelease {
            name: name.to_owned(),
            version: version.to_owned(),
        };
        let outcome = check_link(
            &transport,
            "t",
            &repo,
            &release,
            &dir,
            today(),
            &mut RecordingPacer::default(),
        );
        assert!(
            matches!(outcome, Err(LinkError::UnsafeName { .. })),
            "{name} {version}"
        );
    }
    assert!(transport.urls().is_empty());
}
