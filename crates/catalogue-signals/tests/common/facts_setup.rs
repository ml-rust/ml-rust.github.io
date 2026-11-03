//! Shared setup for the repo-facts tests: a pacer, fixed inputs and readers.

use catalogue_signals::github::{Facts, GithubError, Pacer, RepoFacts, RepoRef, repo_facts};
use catalogue_signals::http::Headers;
use jiff::civil::{Date, date};

use crate::facts_responses::RepoBody;
use crate::transport::FakeTransport;

/// A pacer that records the `retry-after` of each response it waits on.
#[derive(Default)]
pub struct RecordingPacer {
    pub waits: Vec<Option<String>>,
}

impl Pacer for RecordingPacer {
    fn wait(&mut self, response_headers: &Headers) {
        self.waits
            .push(response_headers.get("retry-after").map(str::to_owned));
    }
}

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
