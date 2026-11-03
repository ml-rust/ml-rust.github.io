//! Public account facts: the owner of a repo and public org membership.

use catalogue_signals::github::RepoRef;
use catalogue_signals::http::percent_encode;
use serde::Deserialize;

use crate::api::{API_URL, Verb, parse_json};
use crate::client::RepoClient;
use crate::error::ApiError;
use crate::issues::Author;

/// The repo fields the client reads.
#[derive(Deserialize)]
struct RawRepo {
    owner: Author,
}

impl RepoClient<'_> {
    /// The owner of `repo`, or `None` when the repo is not found.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or the answer is not the expected JSON.
    pub fn repo_owner(&mut self, repo: &RepoRef) -> Result<Option<Author>, ApiError> {
        let url = format!(
            "{API_URL}/repos/{}/{}",
            percent_encode(&repo.owner),
            percent_encode(&repo.name)
        );
        let answer = self.call(Verb::Get, &url, None, &[200, 404])?;
        if answer.status == 404 {
            return Ok(None);
        }
        let raw: RawRepo = parse_json(&url, &answer.body)?;
        Ok(Some(raw.owner))
    }

    /// Whether `user` is a public member of `org`: 204 yes, 404 no.
    ///
    /// The call reads public membership only. It never reads a private member list.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or GitHub answers with
    /// another status.
    pub fn public_member(&mut self, org: &str, user: &str) -> Result<bool, ApiError> {
        let url = format!(
            "{API_URL}/orgs/{}/public_members/{}",
            percent_encode(org),
            percent_encode(user)
        );
        let answer = self.call(Verb::Get, &url, None, &[204, 404])?;
        Ok(answer.status == 204)
    }
}
