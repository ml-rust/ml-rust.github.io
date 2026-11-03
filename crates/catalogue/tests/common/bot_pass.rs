//! Runs one `catalogue bot run` pass over a scripted GitHub.

use std::path::PathBuf;
use std::time::Duration;

use catalogue::{PassDeps, RunError, RunReport, TriageDeps, run_pass};
use catalogue_github::testing::{NoWait, ScriptedTransport};
use catalogue_triage::Backoff;
use jiff::Timestamp;

use super::bot_github::assert_redacted;
use super::bot_setup::BotSetup;

/// The clock of every pass unless a test sets another.
pub fn now() -> Timestamp {
    "2026-11-04T00:00:00Z".parse().expect("time")
}

/// The state directory of `setup`.
pub fn state_dir(setup: &BotSetup) -> PathBuf {
    setup.config.state_dir.clone().expect("state dir")
}

/// A [`Backoff`] that never waits.
struct NoBackoff;

impl Backoff for NoBackoff {
    fn pause(&mut self, _wait: Duration) {}
}

/// One pass at `at`, with pacers that never wait and the variables in `env`.
///
/// The link check sends its requests over `transport` too, so a test sees
/// every request in one list.
pub fn try_run_at(
    setup: &BotSetup,
    transport: &ScriptedTransport,
    env: &[(&str, &str)],
    at: Timestamp,
) -> Result<RunReport, RunError> {
    let mut pacer = NoWait::default();
    let mut crates_pacer = NoWait::default();
    let mut llm_pacer = NoWait::default();
    let mut fetch_pacer = NoWait::default();
    let mut backoff = NoBackoff;
    let clock = move || at;
    let lookup = |name: &str| {
        env.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    };
    let deps = PassDeps {
        transport,
        pacer: &mut pacer,
        crates_pacer: &mut crates_pacer,
        clock: &clock,
        triage: TriageDeps {
            transport,
            llm_pacer: &mut llm_pacer,
            fetch_pacer: &mut fetch_pacer,
            backoff: &mut backoff,
            env: &lookup,
        },
    };
    run_pass(deps, &setup.config, &setup.labels, &state_dir(setup))
}

/// One pass with pacers that never wait and no key variable.
pub fn try_run(setup: &BotSetup, transport: &ScriptedTransport) -> Result<RunReport, RunError> {
    try_run_at(setup, transport, &[], now())
}

/// One pass that must succeed, with every request checked for the agent and a redacted token.
pub fn run(setup: &BotSetup, transport: &ScriptedTransport) -> RunReport {
    let report = try_run(setup, transport).expect("runs");
    assert_redacted(transport);
    report
}
