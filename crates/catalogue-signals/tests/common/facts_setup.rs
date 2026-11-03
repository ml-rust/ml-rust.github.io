//! Shared setup for the repo-facts tests: fixed inputs and readers.

use catalogue_signals::github::{Facts, GithubError, RepoFacts, RepoRef, repo_facts};
use jiff::civil::{Date, date};

use crate::facts_responses::RepoBody;
use crate::recording_pacer::RecordingPacer;
use crate::transport::FakeTransport;

pub const TODAY: Date = date(2026, 11, 3);
pub const TOKEN: &str = "sekret-xyz";

pub fn requested() -> RepoRef {
    RepoRef::parse("a/b").expect("parses")
}

pub fn read(transport: &FakeTransport) -> Result<RepoFacts, GithubError> {
    repo_facts(
        transport,
        TOKEN,
        &requested(),
        TODAY,
        &mut RecordingPacer::default(),
    )
}

pub fn found(body: &RepoBody) -> Facts {
    let transport = FakeTransport::new([body.response()]);
    match read(&transport).expect("reads") {
        RepoFacts::Found(facts) => facts,
        RepoFacts::Gone { .. } => panic!("expected a found repo"),
    }
}
