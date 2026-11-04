//! Shared setup for the link-check tests: a fixed release, canned responses and a runner.

use std::path::Path;

use catalogue_core::Date;
use catalogue_signals::github::RepoRef;
use catalogue_signals::link::{CrateRelease, LinkCheck, LinkError, LinkOutcome, check_link};

use crate::recording_pacer::RecordingPacer;
use crate::transport::{FakeResponse, FakeTransport};

pub const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";

/// The run date of every check.
pub fn today() -> Date {
    Date::new(2026, 11, 3).expect("date")
}

fn release() -> CrateRelease {
    CrateRelease {
        name: "Serde_Json".to_owned(),
        version: "1.0.1".to_owned(),
    }
}

pub fn vcs(dirty: bool) -> String {
    format!(r#"{{"git":{{"sha1":"{SHA}","dirty":{dirty}}},"path_in_vcs":""}}"#)
}

pub fn package(json: Option<&str>) -> FakeResponse {
    FakeResponse::package("Serde_Json", "1.0.1", json)
}

/// The GitHub lookup that finds the commit and compares it with the default branch as `status`.
pub fn compared(status: &str) -> Vec<FakeResponse> {
    vec![
        FakeResponse::with_body(
            200,
            &[],
            r#"{"data":{"repository":{"object":{"__typename":"Commit"}}}}"#,
        ),
        FakeResponse::with_body(
            200,
            &[],
            &format!(
                r#"{{"data":{{"repository":{{"defaultBranchRef":{{"name":"main","compare":{{"status":"{status}"}}}},"tag0":null,"tag1":null,"tag2":null,"tag3":null}}}}}}"#
            ),
        ),
    ]
}

/// The GitHub lookup that does not know the commit.
pub fn commit_absent() -> Vec<FakeResponse> {
    vec![FakeResponse::with_body(
        200,
        &[],
        r#"{"data":{"repository":{"object":null}}}"#,
    )]
}

/// The GitHub lookup whose first request answers with only an HTTP `code`.
pub fn lookup_code(code: u16) -> Vec<FakeResponse> {
    vec![FakeResponse::status(code)]
}

/// The package download followed by the GitHub lookup.
pub fn then(first: FakeResponse, rest: Vec<FakeResponse>) -> Vec<FakeResponse> {
    std::iter::once(first).chain(rest).collect()
}

pub fn run(
    responses: Vec<FakeResponse>,
    repo: &str,
    cache_dir: &Path,
) -> (FakeTransport, Result<LinkOutcome, LinkError>) {
    let transport = FakeTransport::new(responses);
    let repo = RepoRef::parse(repo).expect("repo");
    let outcome = check_link(
        &transport,
        "sekret-xyz",
        &repo,
        &release(),
        cache_dir,
        today(),
        &mut RecordingPacer::default(),
    );
    (transport, outcome)
}

pub fn verified(dirty: bool) -> LinkCheck {
    LinkCheck::Verified {
        sha1: SHA.to_owned(),
        dirty,
        held_by: Some("refs/heads/main".to_owned()),
    }
}
