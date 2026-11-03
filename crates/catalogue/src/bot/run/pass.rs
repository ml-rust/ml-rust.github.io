//! One poll pass over the open issues.

use std::collections::BTreeSet;
use std::path::Path;

use catalogue_core::Date;
use catalogue_github::{AppKey, Clock, GithubApp};
use catalogue_signals::http::{Pacer, Transport};
use jiff::tz::TimeZone;

use super::error::RunError;
use super::issue::{RunContext, check_issue, has_form_label};
use super::lock::take_lock;
use super::state::{BotState, STATE_FILE};
use crate::bot::config::BotConfig;
use crate::bot::labels::{
    LabelsCommandError, LabelsReport, permissions, read_labels_file, sync_with,
};

/// The branch that holds the data files the checks read.
pub const MAIN_BRANCH: &str = "main";

/// The branch whose root holds the daily tier snapshots.
pub const SIGNALS_BRANCH: &str = "signals";

/// What one pass did.
#[derive(Debug)]
pub struct RunReport {
    /// The label sync the pass applied first.
    pub labels: LabelsReport,
    /// One line per checked issue: the number and what changed.
    pub lines: Vec<String>,
    /// Problems that did not stop the pass, such as an ignored state file.
    pub warnings: Vec<String>,
    /// Issues the pass did not finish, and state writes that failed.
    pub failures: Vec<String>,
}

/// Runs one poll pass for the repo in `config`.
///
/// The pass takes the lock in `state_dir`, syncs the labels to the file at
/// `labels_path`, reads the data files of `main` and the newest tier
/// snapshot, then checks every open issue with a form label. It writes the
/// state file after each issue and drops issues that left the work set. An
/// issue that fails is reported and the pass goes on. The `pacer` spaces every
/// request. The `clock` signs the JWTs and gives the date of the check.
///
/// # Errors
///
/// Returns a [`RunError`] when the lock is held, the labels file or the key
/// cannot be used, or the label sync or a fetch the whole pass needs fails.
pub fn run_pass(
    transport: &dyn Transport,
    pacer: &mut dyn Pacer,
    clock: Clock<'_>,
    config: &BotConfig,
    labels_path: &Path,
    state_dir: &Path,
) -> Result<RunReport, RunError> {
    let _lock = take_lock(state_dir)?;
    let file = read_labels_file(labels_path)?;
    let key = AppKey::read(&config.private_key_path).map_err(LabelsCommandError::from)?;
    let state_path = state_dir.join(STATE_FILE);
    let (mut state, warning) = BotState::load(&state_path);
    let app = GithubApp::new(transport, pacer, config.app_id.get(), key, clock);
    let mut client = app.repo(
        config.installation_id.get(),
        config.repo.clone(),
        permissions(),
    );
    let slug = client.app().slug()?;
    let labels = sync_with(&mut client, &file, false).map_err(LabelsCommandError::from)?;
    let main = client.branch_data(MAIN_BRANCH)?;
    let snapshot = client.newest_snapshot(SIGNALS_BRANCH)?;
    let issues = client.open_issues()?;
    let context = RunContext {
        slug: &slug,
        main: &main.sources,
        main_sha: &main.sha,
        snapshot: snapshot.as_ref().map(|found| &found.snapshot),
        today: Date::from(clock().to_zoned(TimeZone::UTC).date()),
    };
    let mut report = RunReport {
        labels,
        lines: Vec::new(),
        warnings: warning.into_iter().collect(),
        failures: Vec::new(),
    };
    let mut work = BTreeSet::new();
    for issue in issues.iter().filter(|issue| has_form_label(issue)) {
        work.insert(issue.number);
        match check_issue(&mut client, &context, issue, state.get(issue.number)) {
            Ok(checked) => {
                report.lines.push(checked.summary);
                if state.get(issue.number) != Some(&checked.state) {
                    state.insert(issue.number, checked.state);
                    save(&state, &state_path, &mut report);
                }
            }
            Err(error) => report.failures.push(error.to_string()),
        }
    }
    let before = state.clone();
    state.retain(&work);
    if state != before {
        save(&state, &state_path, &mut report);
    }
    Ok(report)
}

/// Writes `state` to `path`. A failed write becomes a failure of the pass.
fn save(state: &BotState, path: &Path, report: &mut RunReport) {
    if let Err(error) = state.save(path) {
        report.failures.push(error.to_string());
    }
}
