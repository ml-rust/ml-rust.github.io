#[path = "common/cache.rs"]
mod cache;
#[path = "common/link_setup.rs"]
mod link_setup;
#[path = "common/package_responses.rs"]
mod package_responses;
#[path = "common/recording_pacer.rs"]
mod recording_pacer;
#[path = "common/request_urls.rs"]
mod request_urls;
#[path = "common/response_status.rs"]
mod response_status;
#[path = "common/search_responses.rs"]
mod search_responses;
#[path = "common/transport.rs"]
mod transport;

use std::fs;

use cache::cache;
use catalogue_signals::github::GithubError;
use catalogue_signals::link::{LinkCheck, LinkError, links_dir};
use link_setup::{SHA, compare_code, compared, package, run, then, today, vcs, verified};
use transport::FakeResponse;

#[test]
fn a_second_run_reads_the_cache_and_sends_nothing() {
    let (_guard, dir) = cache();
    let (_, first) = run(
        then(package(Some(&vcs(false))), compared("ahead")),
        "a/b",
        &dir,
    );
    let first = first.expect("checks");
    assert!(!first.cached);
    assert_eq!(first.checked, today());
    assert!(links_dir(&dir).join("serde-json-1.0.1@a__b.json").is_file());
    let (transport, second) = run(vec![], "A/B", &dir);
    let second = second.expect("cache hit");
    assert!(second.cached);
    assert_eq!(second.check, verified(false));
    assert_eq!(second.checked, today());
    assert!(transport.urls().is_empty());
}

#[test]
fn a_changed_repo_checks_again_and_keeps_the_first_repos_file() {
    let (_guard, dir) = cache();
    run(
        then(package(Some(&vcs(false))), compared("ahead")),
        "a/b",
        &dir,
    )
    .1
    .expect("first");
    let responses = then(package(Some(&vcs(false))), compare_code(404));
    let (transport, other) = run(responses, "c/d", &dir);
    let other = other.expect("second");
    assert!(!other.cached);
    assert!(matches!(other.check, LinkCheck::CommitMissing { .. }));
    assert_eq!(transport.urls().len(), 3);
    let (transport, again) = run(vec![], "c/d", &dir);
    assert!(again.expect("hit").cached);
    assert!(transport.urls().is_empty());
    let (transport, first_again) = run(vec![], "a/b", &dir);
    let first_again = first_again.expect("first repo still hits");
    assert!(first_again.cached);
    assert_eq!(first_again.check, verified(false));
    assert!(transport.urls().is_empty());
}

#[test]
fn a_package_error_is_not_cached() {
    let (_guard, dir) = cache();
    let (_, outcome) = run(vec![FakeResponse::status(404)], "a/b", &dir);
    assert!(matches!(
        outcome,
        Err(LinkError::Download { status: 404, .. })
    ));
    assert!(!links_dir(&dir).join("serde-json-1.0.1@a__b.json").exists());
}

#[test]
fn a_result_from_the_first_rule_is_checked_again() {
    let (_guard, dir) = cache();
    fs::create_dir_all(links_dir(&dir)).expect("dir");
    let old =
        format!(r#"{{"repo":"a/b","result":{{"kind":"verified","sha1":"{SHA}","dirty":false}}}}"#);
    fs::write(links_dir(&dir).join("serde-json-1.0.1@a__b.json"), old).expect("file");
    let responses = then(package(Some(&vcs(false))), compare_code(404));
    let (transport, outcome) = run(responses, "a/b", &dir);
    let outcome = outcome.expect("checks");
    assert!(!outcome.cached);
    assert!(matches!(outcome.check, LinkCheck::CommitMissing { .. }));
    assert_eq!(transport.urls().len(), 3);
}

#[test]
fn an_unreadable_cache_file_is_checked_again() {
    let (_guard, dir) = cache();
    fs::create_dir_all(links_dir(&dir)).expect("dir");
    fs::write(
        links_dir(&dir).join("serde-json-1.0.1@a__b.json"),
        "not json",
    )
    .expect("file");
    let (_, outcome) = run(vec![package(None)], "a/b", &dir);
    let outcome = outcome.expect("checks");
    assert!(!outcome.cached);
    let (_, hit) = run(vec![], "a/b", &dir);
    assert_eq!(hit.expect("hit").check, LinkCheck::NoVcsInfo);
}

#[test]
fn errors_are_not_cached() {
    let (_guard, dir) = cache();
    let (_, outcome) = run(
        then(package(Some(&vcs(false))), compare_code(500)),
        "a/b",
        &dir,
    );
    assert!(matches!(
        outcome,
        Err(LinkError::Github(GithubError::Status { status: 500, .. }))
    ));
    assert!(!links_dir(&dir).join("serde-json-1.0.1@a__b.json").exists());
}
