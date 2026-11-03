//! Runs one `catalogue bot run` pass over a scripted GitHub.

use std::path::PathBuf;

use catalogue::{RunError, RunReport, run_pass};
use catalogue_github::testing::{NoWait, ScriptedTransport};
use jiff::Timestamp;

use super::bot_github::assert_redacted;
use super::bot_setup::BotSetup;

/// The clock of every pass.
fn now() -> Timestamp {
    "2026-11-04T00:00:00Z".parse().expect("time")
}

/// The state directory of `setup`.
pub fn state_dir(setup: &BotSetup) -> PathBuf {
    setup.config.state_dir.clone().expect("state dir")
}

/// One pass with pacers that never wait.
pub fn try_run(setup: &BotSetup, transport: &ScriptedTransport) -> Result<RunReport, RunError> {
    let mut pacer = NoWait::default();
    let mut crates_pacer = NoWait::default();
    run_pass(
        transport,
        &mut pacer,
        &mut crates_pacer,
        &now,
        &setup.config,
        &setup.labels,
        &state_dir(setup),
    )
}

/// One pass that must succeed, with every request checked for the agent and a redacted token.
pub fn run(setup: &BotSetup, transport: &ScriptedTransport) -> RunReport {
    let report = try_run(setup, transport).expect("runs");
    assert_redacted(transport);
    report
}
