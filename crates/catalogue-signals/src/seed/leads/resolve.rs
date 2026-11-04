//! The GitHub batch of a lead run.

use crate::github::{GithubError, RepoProfile, RepoRef, repo_profile};
use crate::http::{Headers, Pacer, Transport};

/// What GitHub answers for one lead.
pub(super) enum Answer {
    /// The repo exists.
    Found(RepoProfile),
    /// GitHub holds no repo of that name.
    Gone,
    /// The fetch failed. The text names the error and the next action.
    Failed(String),
}

/// Reads the profile of each lead in order, with one pacer across the batch.
///
/// A failed fetch stays on its own lead. A 401 stops the batch, because no later request can
/// pass with that token.
pub(super) fn resolve(
    transport: &dyn Transport,
    token: &str,
    leads: &[RepoRef],
    pacer: &mut dyn Pacer,
) -> Result<Vec<Answer>, GithubError> {
    let mut answers = Vec::with_capacity(leads.len());
    for (index, lead) in leads.iter().enumerate() {
        if index > 0 {
            pacer.wait(&Headers::default());
        }
        answers.push(match repo_profile(transport, token, lead, pacer) {
            Ok(Some(profile)) => Answer::Found(profile),
            Ok(None) => Answer::Gone,
            Err(error @ GithubError::Status { status: 401, .. }) => return Err(error),
            Err(error) => Answer::Failed(error.to_string()),
        });
    }
    Ok(answers)
}
