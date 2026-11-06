//! Commits the state report to the `signals` branch.

use catalogue_github::{ApiError, RepoClient};

use crate::bot::run::SIGNALS_BRANCH;

/// The report on the `signals` branch. The site build reads it.
pub const STATE_PATH: &str = "state/state.json";

/// The tries when the Pages job moves the branch at the same moment.
const ATTEMPTS: u32 = 5;

/// Replaces the report on the `signals` branch with `text`. Returns whether the branch moved.
///
/// The branch holds the daily snapshots, so each try builds on its current head and the branch
/// only ever moves forward. A head that moved meanwhile starts the next try.
///
/// # Errors
///
/// Returns an [`ApiError`] when a call fails, or when the branch keeps moving.
pub fn publish_state(client: &mut RepoClient<'_>, text: &str) -> Result<bool, ApiError> {
    let mut outcome = Ok(false);
    for _ in 0..ATTEMPTS {
        let head = client.branch_sha(SIGNALS_BRANCH)?;
        let base = client.commit_tree(&head)?;
        let tree = client.create_tree(&base, STATE_PATH, text)?;
        if tree == base {
            return Ok(false);
        }
        let commit = client.create_commit("Update the state report", &tree, &head)?;
        match client.advance_branch(SIGNALS_BRANCH, &commit) {
            Ok(true) => return Ok(true),
            Ok(false) => outcome = Ok(false),
            Err(error) => outcome = Err(error),
        }
    }
    outcome?;
    Err(ApiError::Content {
        url: SIGNALS_BRANCH.to_owned(),
        reason: format!("the branch moved during all {ATTEMPTS} tries. Rerun later"),
    })
}
