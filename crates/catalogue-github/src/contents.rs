//! Repo contents: a branch head, the data files of a commit and the newest tier snapshot.

use std::path::PathBuf;

use catalogue_core::{CommitSha, DataSources};
use catalogue_signals::history::{Snapshot, read_snapshot, snapshot_date};
use catalogue_signals::http::percent_encode;
use catalogue_signals::repo_archive::read_repo_data;
use serde::Deserialize;

use crate::api::{Verb, parse_json};
use crate::client::RepoClient;
use crate::error::ApiError;

/// The media type that asks for a commit SHA as plain text.
const ACCEPT_SHA: &str = "application/vnd.github.sha";

/// The media type that asks for raw file bytes.
const ACCEPT_RAW: &str = "application/vnd.github.raw";

/// The most compressed bytes a repo archive holds.
pub const MAX_ARCHIVE_BYTES: usize = 24 << 20;

/// One entry of a git tree.
#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
}

/// A git tree, as the API lists it.
#[derive(Deserialize)]
struct Tree {
    tree: Vec<TreeEntry>,
    #[serde(default)]
    truncated: bool,
}

/// The data files of one commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitData {
    /// The commit SHA.
    pub sha: CommitSha,
    /// The data files under `data/` at the commit.
    pub sources: DataSources,
}

/// The newest tier snapshot on a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct BranchSnapshot {
    /// The snapshot file name, `YYYY-MM-DD.jsonl.gz`.
    pub name: String,
    /// The snapshot rows.
    pub snapshot: Snapshot,
}

impl RepoClient<'_> {
    /// The full SHA of the commit at the head of `branch`.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or the answer is not a full
    /// lowercase hex SHA.
    pub fn branch_sha(&mut self, branch: &str) -> Result<CommitSha, ApiError> {
        let url = self.repo_url(&format!("/commits/{}", percent_encode(branch)));
        let answer = self.get_accept(&url, ACCEPT_SHA, &[200])?;
        let text = String::from_utf8_lossy(&answer.body).trim().to_owned();
        CommitSha::new(text).map_err(|error| ApiError::Content {
            url,
            reason: format!("{error}. Rerun later. If the error persists, report it"),
        })
    }

    /// The data files at the head of `branch`.
    ///
    /// The client reads the head SHA first, then the archive of that commit,
    /// so the files and the SHA always match. The transport follows the
    /// redirect to the archive host.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when a call fails, the archive passes
    /// [`MAX_ARCHIVE_BYTES`], or the archive breaks a rule of
    /// [`read_repo_data`].
    pub fn branch_data(&mut self, branch: &str) -> Result<CommitData, ApiError> {
        let sha = self.branch_sha(branch)?;
        let url = self.repo_url(&format!("/tarball/{sha}"));
        let answer = self.call(Verb::Get, &url, None, &[200])?;
        if answer.body.len() > MAX_ARCHIVE_BYTES {
            return Err(ApiError::Content {
                url,
                reason: format!(
                    "the archive passes the {MAX_ARCHIVE_BYTES}-byte cap. Report it, so the cap can be checked"
                ),
            });
        }
        let sources = read_repo_data(answer.body.as_slice())
            .map_err(|source| ApiError::Archive { url, source })?;
        Ok(CommitData { sha, sources })
    }

    /// The newest `YYYY-MM-DD.jsonl.gz` snapshot at the root of `branch`.
    ///
    /// `None` when the branch does not exist or holds no snapshot.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when a call fails, the tree listing is cut
    /// short, or the snapshot is corrupt.
    pub fn newest_snapshot(&mut self, branch: &str) -> Result<Option<BranchSnapshot>, ApiError> {
        let tree_url = self.repo_url(&format!("/git/trees/{}", percent_encode(branch)));
        let answer = self.call(Verb::Get, &tree_url, None, &[200, 404])?;
        if answer.status == 404 {
            return Ok(None);
        }
        let tree: Tree = parse_json(&tree_url, &answer.body)?;
        if tree.truncated {
            return Err(ApiError::Content {
                url: tree_url,
                reason: "the tree listing is cut short. Report it".to_owned(),
            });
        }
        let newest = tree
            .tree
            .into_iter()
            .filter(|entry| entry.kind == "blob")
            .filter_map(|entry| snapshot_date(&entry.path).map(|date| (date, entry.path)))
            .max_by_key(|(date, _)| *date);
        let Some((date, name)) = newest else {
            return Ok(None);
        };
        let url = self.repo_url(&format!(
            "/contents/{}?ref={}",
            percent_encode(&name),
            percent_encode(branch)
        ));
        let answer = self.get_accept(&url, ACCEPT_RAW, &[200, 404])?;
        if answer.status == 404 {
            return Ok(None);
        }
        let label = PathBuf::from(format!("{branch}:{name}"));
        let snapshot = read_snapshot(answer.body.as_slice(), &label, date)
            .map_err(|source| ApiError::Snapshot { url, source })?;
        Ok(Some(BranchSnapshot { name, snapshot }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_listing_parses() {
        let tree: Tree = serde_json::from_str(
            r#"{"sha":"x","tree":[{"path":"2026-11-03.jsonl.gz","type":"blob","sha":"y"}],"truncated":false}"#,
        )
        .expect("parses");
        assert_eq!(tree.tree.len(), 1);
        assert!(!tree.truncated);
    }
}
