//! Recorded-shape GraphQL answers for the repo facts.

use serde_json::{Value, json};

use crate::transport::FakeResponse;

/// A commit node.
pub fn commit(when: &str, login: Option<&str>, name: &str) -> Value {
    json!({
        "committedDate": when,
        "author": {"user": login.map(|login| json!({"login": login})), "name": name},
    })
}

/// An issue or pull request node.
pub fn item(
    created: &str,
    state: &str,
    association: &str,
    login: &str,
    comments: Vec<Value>,
) -> Value {
    json!({
        "createdAt": created,
        "state": state,
        "authorAssociation": association,
        "author": {"login": login},
        "comments": {"nodes": comments},
    })
}

/// The body of a found repo.
pub struct RepoBody {
    pub name: String,
    pub ci: Option<&'static str>,
    pub total_commits: u64,
    /// The commits of the dated window.
    pub commits: Vec<Value>,
    /// The commits of the unbounded latest sample.
    pub latest_commits: Vec<Value>,
    /// The commits on the branch in all.
    pub latest_total: u64,
    pub issues: Vec<Value>,
    pub pulls: Vec<Value>,
}

impl RepoBody {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ci: Some("SUCCESS"),
            total_commits: 0,
            commits: Vec::new(),
            latest_commits: Vec::new(),
            latest_total: 0,
            issues: Vec::new(),
            pulls: Vec::new(),
        }
    }

    /// The JSON text of the answer.
    pub fn text(&self) -> String {
        json!({
            "data": {
                "repository": {
                    "nameWithOwner": self.name,
                    "isArchived": false,
                    "isFork": true,
                    "pushedAt": "2026-10-26T17:14:42Z",
                    "stargazerCount": 64548,
                    "forkCount": 8123,
                    "defaultBranchRef": {
                        "name": "main",
                        "target": {
                            "statusCheckRollup": self.ci.map(|state| json!({"state": state})),
                            "recent": {"totalCount": self.total_commits, "nodes": self.commits},
                            "latest": {"totalCount": self.latest_total, "nodes": self.latest_commits},
                        },
                    },
                    "issues": {"nodes": self.issues},
                    "pullRequests": {"nodes": self.pulls},
                },
                "rateLimit": {"cost": 1, "remaining": 4999, "resetAt": "2026-11-03T13:25:38Z"},
            },
        })
        .to_string()
    }

    /// A 200 response with the answer.
    pub fn response(&self) -> FakeResponse {
        FakeResponse::with_body(200, &[], &self.text())
    }
}
