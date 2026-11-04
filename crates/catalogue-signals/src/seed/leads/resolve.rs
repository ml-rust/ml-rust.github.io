//! The GitHub batch of a lead run.

use super::decide::is_rust;
use super::languages::lists_rust;
use crate::github::{GithubError, RepoProfile, RepoRef, repo_profile};
use crate::http::{Headers, Pacer, Transport};

/// An existing repo, with what its language list says.
pub(super) struct Found {
    pub(super) profile: RepoProfile,
    /// GitHub lists Rust among the languages. False for a Rust repo, which needs no list.
    pub(super) lists_rust: bool,
    /// The language list fetch failed. The text names the error.
    pub(super) warning: Option<String>,
}

/// What GitHub answers for one lead.
pub(super) enum Answer {
    /// The repo exists.
    Found(Found),
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
            Ok(Some(profile)) => {
                let (rust, warning) = read_languages(transport, token, lead, &profile, pacer)?;
                Answer::Found(Found {
                    profile,
                    lists_rust: rust,
                    warning,
                })
            }
            Ok(None) => Answer::Gone,
            Err(error @ GithubError::Status { status: 401, .. }) => return Err(error),
            Err(error) => Answer::Failed(error.to_string()),
        });
    }
    Ok(answers)
}

/// Reads the language list of a repo whose primary language is not Rust.
///
/// A failed fetch reads as no Rust and adds a warning. A 401 stops the batch.
fn read_languages(
    transport: &dyn Transport,
    token: &str,
    lead: &RepoRef,
    profile: &RepoProfile,
    pacer: &mut dyn Pacer,
) -> Result<(bool, Option<String>), GithubError> {
    if is_rust(profile) {
        return Ok((false, None));
    }
    pacer.wait(&Headers::default());
    let repo = RepoRef::parse(&profile.name_with_owner).unwrap_or_else(|_| lead.clone());
    match lists_rust(transport, token, &repo, pacer) {
        Ok(found) => Ok((found, None)),
        Err(error @ GithubError::Status { status: 401, .. }) => Err(error),
        Err(error) => Ok((
            false,
            Some(format!(
                "{repo}: the language list fetch failed, so the primary language decides alone: {error}"
            )),
        )),
    }
}
