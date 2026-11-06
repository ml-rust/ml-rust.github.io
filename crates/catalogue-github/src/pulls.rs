//! Pull requests and the git data calls that build one commit on a branch.

use catalogue_core::CommitSha;
use catalogue_signals::http::percent_encode;
use jiff::Timestamp;
use serde::Deserialize;
use serde_json::json;

use crate::api::{PER_PAGE, Verb, parse_json};
use crate::client::RepoClient;
use crate::error::ApiError;

/// The mode of a regular file in a git tree.
const FILE_MODE: &str = "100644";

/// A pull request, as the listing reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pull {
    /// The PR number.
    pub number: u64,
    /// Whether the PR is open.
    pub open: bool,
    /// Whether the PR is merged.
    pub merged: bool,
    /// When the PR closed. `None` while open.
    pub closed_at: Option<Timestamp>,
    /// The head branch name.
    pub head_ref: String,
}

/// A new pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPull<'a> {
    /// The title.
    pub title: &'a str,
    /// The body.
    pub body: &'a str,
    /// The head branch in this repo.
    pub head: &'a str,
    /// The base branch.
    pub base: &'a str,
}

/// How [`RepoClient::point_branch`] set the branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchWrite {
    /// The branch is new.
    Created,
    /// The branch existed and now points at the commit.
    Moved,
}

/// The PR fields the client reads.
#[derive(Deserialize)]
struct RawPull {
    number: u64,
    state: String,
    #[serde(default)]
    merged_at: Option<String>,
    #[serde(default)]
    closed_at: Option<String>,
    head: RawHead,
}

/// The head of a PR.
#[derive(Deserialize)]
struct RawHead {
    #[serde(rename = "ref")]
    name: String,
}

/// An object that names itself by SHA.
#[derive(Deserialize)]
struct Named {
    sha: String,
}

/// A commit, with its tree.
#[derive(Deserialize)]
struct RawCommit {
    tree: Named,
}

/// A created PR.
#[derive(Deserialize)]
struct Created {
    number: u64,
}

/// The ref path of `branch`, each `/`-separated part percent-encoded.
fn ref_path(branch: &str) -> String {
    branch
        .split('/')
        .map(percent_encode)
        .collect::<Vec<_>>()
        .join("/")
}

/// The SHA in `bytes` from `url`, checked as a full lowercase hex SHA.
fn sha_of(url: &str, bytes: &[u8]) -> Result<CommitSha, ApiError> {
    let named: Named = parse_json(url, bytes)?;
    CommitSha::new(named.sha).map_err(|error| ApiError::Content {
        url: url.to_owned(),
        reason: format!("{error}. Rerun later. If the error persists, report it"),
    })
}

impl RepoClient<'_> {
    /// Every PR, open or closed, whose head is `branch` in this repo.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when a call fails, a page is not the expected
    /// JSON, a close time is not a timestamp, or the listing passes the page cap.
    pub fn pulls_for_branch(&mut self, branch: &str) -> Result<Vec<Pull>, ApiError> {
        let head = format!("{}:{branch}", self.repo().owner);
        let url = self.repo_url(&format!(
            "/pulls?head={}&state=all&per_page={PER_PAGE}",
            percent_encode(&head)
        ));
        let raw: Vec<RawPull> = self.get_pages(&url)?;
        raw.into_iter()
            .map(|pull| {
                let closed_at = match pull.closed_at {
                    Some(text) => Some(text.parse::<Timestamp>().map_err(|_| {
                        ApiError::Content {
                            url: url.clone(),
                            reason: format!(
                                "PR {} has no valid `closed_at`. Rerun later. If the error persists, report it",
                                pull.number
                            ),
                        }
                    })?),
                    None => None,
                };
                Ok(Pull {
                    number: pull.number,
                    open: pull.state == "open",
                    merged: pull.merged_at.is_some(),
                    closed_at,
                    head_ref: pull.head.name,
                })
            })
            .collect()
    }

    /// The tree SHA of commit `sha`.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or the answer holds no valid tree SHA.
    pub fn commit_tree(&mut self, sha: &CommitSha) -> Result<CommitSha, ApiError> {
        let url = self.repo_url(&format!("/git/commits/{sha}"));
        let answer = self.call(Verb::Get, &url, None, &[200])?;
        let commit: RawCommit = parse_json(&url, &answer.body)?;
        CommitSha::new(commit.tree.sha).map_err(|error| ApiError::Content {
            url,
            reason: format!("{error}. Rerun later. If the error persists, report it"),
        })
    }

    /// Creates a tree that holds `base_tree` with the file `path` set to `content`.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or the answer holds no valid SHA.
    pub fn create_tree(
        &mut self,
        base_tree: &CommitSha,
        path: &str,
        content: &str,
    ) -> Result<CommitSha, ApiError> {
        let url = self.repo_url("/git/trees");
        let payload = json!({
            "base_tree": base_tree.as_str(),
            "tree": [{"path": path, "mode": FILE_MODE, "type": "blob", "content": content}],
        })
        .to_string();
        let answer = self.call(Verb::Post, &url, Some(&payload), &[201])?;
        sha_of(&url, &answer.body)
    }

    /// Creates a commit of `tree` on `parent` with `message`.
    ///
    /// The body names no author and no committer, so GitHub records the App.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or the answer holds no valid SHA.
    pub fn create_commit(
        &mut self,
        message: &str,
        tree: &CommitSha,
        parent: &CommitSha,
    ) -> Result<CommitSha, ApiError> {
        let url = self.repo_url("/git/commits");
        let payload = json!({
            "message": message,
            "tree": tree.as_str(),
            "parents": [parent.as_str()],
        })
        .to_string();
        let answer = self.call(Verb::Post, &url, Some(&payload), &[201])?;
        sha_of(&url, &answer.body)
    }

    /// Points `branch` at commit `sha`. Creates the branch, or force-moves it
    /// when it exists. Use it only for a branch the bot owns.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when a call fails.
    pub fn point_branch(&mut self, branch: &str, sha: &CommitSha) -> Result<BranchWrite, ApiError> {
        let url = self.repo_url("/git/refs");
        let payload =
            json!({"ref": format!("refs/heads/{branch}"), "sha": sha.as_str()}).to_string();
        let answer = self.call(Verb::Post, &url, Some(&payload), &[201, 422])?;
        if answer.status == 201 {
            return Ok(BranchWrite::Created);
        }
        let url = self.repo_url(&format!("/git/refs/heads/{}", ref_path(branch)));
        let payload = json!({"sha": sha.as_str(), "force": true}).to_string();
        self.call(Verb::Patch, &url, Some(&payload), &[200])?;
        Ok(BranchWrite::Moved)
    }

    /// Moves existing `branch` forward to commit `sha`, and never force-moves it.
    /// Use it for a branch another writer shares. Returns `false` when `sha` no longer
    /// descends from the head, so the caller builds on the new head and tries again.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails.
    pub fn advance_branch(&mut self, branch: &str, sha: &CommitSha) -> Result<bool, ApiError> {
        let url = self.repo_url(&format!("/git/refs/heads/{}", ref_path(branch)));
        let payload = json!({"sha": sha.as_str(), "force": false}).to_string();
        let answer = self.call(Verb::Patch, &url, Some(&payload), &[200, 422])?;
        Ok(answer.status == 200)
    }

    /// Opens `pull` and returns its number. Maintainers of the base repo cannot
    /// push to the head branch.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or GitHub answers with a
    /// status other than 201.
    pub fn create_pull(&mut self, pull: &NewPull<'_>) -> Result<u64, ApiError> {
        let url = self.repo_url("/pulls");
        let payload = json!({
            "title": pull.title,
            "body": pull.body,
            "head": pull.head,
            "base": pull.base,
            "maintainer_can_modify": false,
        })
        .to_string();
        let answer = self.call(Verb::Post, &url, Some(&payload), &[201])?;
        let created: Created = parse_json(&url, &answer.body)?;
        Ok(created.number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listed_pull_parses() {
        let pull: RawPull = serde_json::from_str(
            r#"{"number":7,"state":"closed","merged_at":null,"closed_at":"2026-11-03T10:00:00Z","head":{"ref":"bot/issue-12","sha":"x"}}"#,
        )
        .expect("parses");
        assert_eq!(pull.head.name, "bot/issue-12");
        assert!(pull.merged_at.is_none());
    }

    #[test]
    fn ref_paths_keep_slashes_and_escape_parts() {
        assert_eq!(ref_path("bot/issue-12"), "bot/issue-12");
        assert_eq!(ref_path("bot/a b"), "bot/a%20b");
    }
}
