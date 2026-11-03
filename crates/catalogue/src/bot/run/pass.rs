//! One poll pass over the open issues.

use std::collections::BTreeSet;
use std::path::Path;

use catalogue_core::Date;
use catalogue_github::{AppKey, Clock, GithubApp};
use catalogue_signals::crates_io::CratesIo;
use catalogue_signals::http::{Pacer, Transport};
use jiff::tz::TimeZone;

use super::error::RunError;
use super::issue::{RunContext, check_issue, has_form_label};
use super::lock::take_lock;
use super::state::{BotState, STATE_FILE};
use super::triage::{LinkCheck, TriageDeps, read_keys};
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
    /// Problems that did not stop the pass, such as an ignored state file, a
    /// provider without a key, or a failed link check.
    pub warnings: Vec<String>,
    /// Issues the pass did not finish, and state writes that failed.
    pub failures: Vec<String>,
}

/// What one pass reads and waits on: the transport, the pacers, the clock and the link check.
pub struct PassDeps<'a> {
    /// Sends the GitHub and crates.io requests.
    pub transport: &'a dyn Transport,
    /// Spaces every GitHub request.
    pub pacer: &'a mut dyn Pacer,
    /// Spaces the crates.io owner lookups.
    pub crates_pacer: &'a mut dyn Pacer,
    /// Signs the JWTs and gives the date and time of the check.
    pub clock: Clock<'a>,
    /// What the link check uses besides the GitHub client.
    pub triage: TriageDeps<'a>,
}

/// Runs one poll pass for the repo in `config`.
///
/// The pass takes the lock in `state_dir`, syncs the labels to the file at
/// `labels_path`, reads the data files of `main` and the newest tier
/// snapshot, then checks every open issue with a form label. An approved
/// issue whose report does not block becomes a data PR. It writes the state
/// file after each issue and drops issues that left the work set. An issue
/// that fails is reported and the pass goes on. With the link check on, the
/// pass reads the key variables first. A provider without a key, or a link
/// check that fails, adds a warning and never stops the pass.
///
/// # Errors
///
/// Returns a [`RunError`] when the lock is held, the labels file or the key
/// cannot be used, or the label sync or a fetch the whole pass needs fails.
pub fn run_pass(
    deps: PassDeps<'_>,
    config: &BotConfig,
    labels_path: &Path,
    state_dir: &Path,
) -> Result<RunReport, RunError> {
    let PassDeps {
        transport,
        pacer,
        crates_pacer,
        clock,
        triage: mut triage_deps,
    } = deps;
    let _lock = take_lock(state_dir)?;
    let file = read_labels_file(labels_path)?;
    let key = AppKey::read(&config.private_key_path).map_err(LabelsCommandError::from)?;
    let state_path = state_dir.join(STATE_FILE);
    let (mut state, warning) = BotState::load(&state_path);
    let mut warnings: Vec<String> = warning.into_iter().collect();
    let keys = read_keys(&config.triage, triage_deps.env, &mut warnings);
    let mut link_check = LinkCheck::start(
        &mut triage_deps,
        keys.as_ref(),
        &config.triage,
        &mut warnings,
    );
    let app = GithubApp::new(transport, pacer, config.app_id.get(), key, clock);
    let mut crates = CratesIo::new(transport, crates_pacer);
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
    let now = clock();
    let context = RunContext {
        slug: &slug,
        main: &main.sources,
        main_sha: &main.sha,
        snapshot: snapshot.as_ref().map(|found| &found.snapshot),
        today: Date::from(now.to_zoned(TimeZone::UTC).date()),
        now: now.as_second(),
    };
    let mut report = RunReport {
        labels,
        lines: Vec::new(),
        warnings,
        failures: Vec::new(),
    };
    let mut work = BTreeSet::new();
    for issue in issues.iter().filter(|issue| has_form_label(issue)) {
        work.insert(issue.number);
        let cached = state.get(issue.number);
        match check_issue(
            &mut client,
            &mut crates,
            &context,
            issue,
            cached,
            link_check.as_mut(),
        ) {
            Ok(checked) => {
                report.lines.push(checked.summary);
                report.warnings.extend(checked.warnings);
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
