//! The parsed GraphQL answer. Unknown fields are ignored.

use serde::Deserialize;

/// The top level of an answer.
#[derive(Debug, Deserialize)]
pub(super) struct Envelope<R = Repository> {
    pub(super) data: Option<Data<R>>,
    #[serde(default)]
    pub(super) errors: Vec<ApiError>,
}

/// One entry of the `errors` array.
#[derive(Debug, Deserialize)]
pub(crate) struct ApiError {
    #[serde(rename = "type")]
    pub(crate) kind: Option<String>,
    #[serde(default)]
    pub(crate) path: Vec<serde_json::Value>,
    #[serde(default)]
    pub(crate) message: String,
}

/// The `data` object.
#[derive(Debug, Deserialize)]
pub(super) struct Data<R = Repository> {
    pub(super) repository: Option<R>,
}

/// A list the API wraps in `nodes`. A node can be null.
#[derive(Debug, Deserialize)]
pub(super) struct Connection<T> {
    pub(super) nodes: Option<Vec<Option<T>>>,
}

impl<T> Connection<T> {
    /// The nodes that are not null.
    pub(super) fn items(&self) -> impl Iterator<Item = &T> {
        self.nodes.iter().flatten().flatten()
    }
}

/// The `repository` object.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Repository {
    pub(super) name_with_owner: String,
    #[serde(default)]
    pub(super) is_archived: bool,
    #[serde(default)]
    pub(super) is_fork: bool,
    pub(super) pushed_at: Option<String>,
    #[serde(default)]
    pub(super) stargazer_count: u32,
    /// The fork count. An answer that lacks it leaves the count unknown, never 0.
    pub(super) fork_count: Option<u32>,
    pub(super) license_info: Option<LicenseInfo>,
    /// The repository topics. An answer that lacks them reads as no topic.
    pub(super) repository_topics: Option<Connection<TopicNode>>,
    pub(super) default_branch_ref: Option<Branch>,
    pub(super) issues: Option<Connection<Item>>,
    pub(super) pull_requests: Option<Connection<Item>>,
}

/// The license GitHub detects in the repo.
#[derive(Debug, Deserialize)]
pub(super) struct LicenseInfo {
    #[serde(rename = "spdxId")]
    pub(super) spdx_id: Option<String>,
}

/// One repository topic link.
#[derive(Debug, Deserialize)]
pub(super) struct TopicNode {
    pub(super) topic: Option<Topic>,
}

/// A topic.
#[derive(Debug, Deserialize)]
pub(super) struct Topic {
    pub(super) name: String,
}

/// The default branch.
#[derive(Debug, Deserialize)]
pub(super) struct Branch {
    pub(super) target: Option<Target>,
}

/// The commit at the branch head.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Target {
    pub(super) status_check_rollup: Option<Rollup>,
    /// The commits since the window start.
    pub(super) recent: Option<History>,
    /// The newest commits, whatever their age.
    pub(super) latest: Option<History>,
}

/// The combined check state.
#[derive(Debug, Deserialize)]
pub(super) struct Rollup {
    pub(super) state: String,
}

/// A list of commits.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct History {
    #[serde(default)]
    pub(super) total_count: u64,
    pub(super) nodes: Option<Vec<Option<Commit>>>,
}

/// One commit.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Commit {
    pub(super) committed_date: String,
    pub(super) author: Option<GitActor>,
}

/// The author of a commit.
#[derive(Debug, Deserialize)]
pub(super) struct GitActor {
    pub(super) user: Option<Login>,
    pub(super) name: Option<String>,
}

/// An account login.
#[derive(Debug, Deserialize)]
pub(super) struct Login {
    pub(super) login: String,
}

/// One issue or pull request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Item {
    pub(super) created_at: String,
    pub(super) state: String,
    pub(super) author_association: Option<String>,
    pub(super) author: Option<Login>,
    pub(super) comments: Option<Connection<Comment>>,
}

/// One comment on an item.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Comment {
    pub(super) created_at: String,
    pub(super) author_association: Option<String>,
}
