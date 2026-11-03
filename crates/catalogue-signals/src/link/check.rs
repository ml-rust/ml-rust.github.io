//! The GitHub default-branch lookup of a release commit.

use serde::Deserialize;

use crate::github::{GithubError, RepoRef, parse_json, rest_headers};
use crate::http::{Paced, Pacer, Transport, percent_encode};

/// The repos endpoint of the GitHub API.
const ENDPOINT: &str = "https://api.github.com/repos";

/// The media type of every lookup.
const ACCEPT: &str = "application/vnd.github+json";

/// Where a commit stands against the default branch of a repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitStanding {
    /// The default branch contains the commit.
    OnDefaultBranch,
    /// The commit exists, but the default branch does not contain it. A commit
    /// that only a fork holds stands here.
    NotOnDefaultBranch,
    /// GitHub does not know the commit or the repo.
    Missing,
}

/// The repo field the lookup reads.
#[derive(Deserialize)]
struct RepoInfo {
    default_branch: String,
}

/// The comparison field the lookup reads.
#[derive(Deserialize)]
struct Comparison {
    status: String,
}

/// Asks GitHub whether the default branch of `repo` contains commit `sha1`.
///
/// The lookup reads the default branch of the repo, then compares `sha1` with
/// that branch. A status of `ahead` or `identical` means the branch contains
/// the commit. Any other status, `behind` and `diverged` included, means it
/// does not. The compare request asks for one commit per page and reads only
/// the status. `token` is sent as a bearer token. A 403 or 429 answer with a
/// wait hint is retried after the `pacer` waits. A 404 or 422 answer means the
/// repo or the commit is missing.
///
/// # Errors
///
/// Returns a [`GithubError`] when no response arrives, a status is not 200,
/// 404 or 422, a body breaks off or is not the expected JSON, or the rate
/// limit stays after the retries.
pub fn commit_standing(
    transport: &dyn Transport,
    token: &str,
    repo: &RepoRef,
    sha1: &str,
    pacer: &mut dyn Pacer,
) -> Result<CommitStanding, GithubError> {
    let headers = rest_headers(token, ACCEPT);
    let mut paced = Paced::new(pacer);
    let repo_url = format!(
        "{ENDPOINT}/{}/{}",
        percent_encode(&repo.owner),
        percent_encode(&repo.name)
    );
    let Some(bytes) = paced.fetch_optional(&repo_url, || transport.get(&repo_url, &headers))?
    else {
        return Ok(CommitStanding::Missing);
    };
    let info: RepoInfo = parse_json(&repo_url, &bytes)?;
    let branch: Vec<String> = info.default_branch.split('/').map(percent_encode).collect();
    let url = format!(
        "{repo_url}/compare/{}...{}?per_page=1",
        percent_encode(sha1),
        branch.join("/")
    );
    let Some(bytes) = paced.fetch_absent(&url, || transport.get(&url, &headers), &[404, 422])?
    else {
        return Ok(CommitStanding::Missing);
    };
    let comparison: Comparison = parse_json(&url, &bytes)?;
    Ok(match comparison.status.as_str() {
        "ahead" | "identical" => CommitStanding::OnDefaultBranch,
        _ => CommitStanding::NotOnDefaultBranch,
    })
}
