//! Scripted routes for the data PR step: issue events, PRs, git data and crates.io owners.

use catalogue_github::testing::{Method, Reply, ScriptedTransport};
use serde_json::{Value, json};

use super::bot_github::{SHA, TOKEN_URL, issue, url};

/// The tree of the `main` commit.
pub const BASE_TREE: &str = "1111111111111111111111111111111111111111";
/// The tree the bot creates.
pub const NEW_TREE: &str = "2222222222222222222222222222222222222222";
/// The commit the bot creates.
pub const COMMIT: &str = "3333333333333333333333333333333333333333";
/// The number of the PR the bot opens.
pub const PR: u64 = 31;
/// The API URL of the `alpha` entry repo.
pub const ALPHA_REPO_URL: &str = "https://api.github.com/repos/ml-rust/alpha";

/// Whether the bot branch exists before the pass.
pub enum Branch {
    /// The branch is new.
    New,
    /// The branch exists, so the create answers 422.
    Exists,
}

/// An open issue by `author` as the API returns it.
pub fn issue_by(number: u64, body: &str, labels: &[&str], author: &str) -> Value {
    let mut found = issue(number, body, labels);
    found["user"]["login"] = json!(author);
    found
}

/// The URL that lists the events of issue `number`.
pub fn events_url(number: u64) -> String {
    url(&format!("/issues/{number}/events?per_page=100"))
}

/// The URL that lists the PRs on the bot branch of issue `number`.
pub fn pulls_url(number: u64) -> String {
    url(&format!(
        "/pulls?head=ml-rust%3Abot%2Fissue-{number}&state=all&per_page=100"
    ))
}

/// The URL that creates a ref.
pub fn refs_url() -> String {
    url("/git/refs")
}

/// The URL of the bot branch ref of issue `number`.
pub fn branch_ref_url(number: u64) -> String {
    url(&format!("/git/refs/heads/bot/issue-{number}"))
}

/// The crates.io URL of the owners of crate `name`.
pub fn owners_url(name: &str) -> String {
    format!("https://crates.io/api/v1/crates/{name}/owners")
}

/// The URL of the public membership of `user` in `org`.
pub fn public_member_url(org: &str, user: &str) -> String {
    format!("https://api.github.com/orgs/{org}/public_members/{user}")
}

/// The event that adds `approved`, by `login` of account type `kind`, at `when`.
pub fn approved(id: u64, login: &str, kind: &str, when: &str) -> Value {
    json!({
        "id": id,
        "event": "labeled",
        "actor": {"login": login, "type": kind},
        "label": {"name": "approved", "color": "2ea44f"},
        "created_at": when,
    })
}

/// A PR on the bot branch of issue `number`.
pub fn pull(
    number: u64,
    state: &str,
    merged_at: Option<&str>,
    closed_at: Option<&str>,
    issue: u64,
) -> Value {
    json!({
        "number": number,
        "state": state,
        "merged_at": merged_at,
        "closed_at": closed_at,
        "head": {"ref": format!("bot/issue-{issue}"), "sha": "x"},
    })
}

/// `transport` with the reads that come before any PR write: the events and the PRs.
pub fn lookups(
    transport: ScriptedTransport,
    number: u64,
    events: &[Value],
    pulls: &[Value],
) -> ScriptedTransport {
    transport
        .route(
            Method::Get,
            &events_url(number),
            Reply::new(200, Value::Array(events.to_vec()).to_string()),
        )
        .route(
            Method::Get,
            &pulls_url(number),
            Reply::new(200, Value::Array(pulls.to_vec()).to_string()),
        )
}

/// `transport` with every route of the PR step: the lookups, the fresh
/// issue, the `main` commit, and the tree, commit, ref and PR writes.
pub fn pr_step(
    transport: ScriptedTransport,
    number: u64,
    fresh: &Value,
    events: &[Value],
    pulls: &[Value],
    branch: &Branch,
) -> ScriptedTransport {
    let transport = lookups(transport, number, events, pulls)
        .route(
            Method::Get,
            &url(&format!("/issues/{number}")),
            Reply::new(200, fresh.to_string()),
        )
        .route(
            Method::Get,
            &url(&format!("/git/commits/{SHA}")),
            Reply::new(
                200,
                json!({"sha": SHA, "tree": {"sha": BASE_TREE}}).to_string(),
            ),
        )
        .route(
            Method::Post,
            &url("/git/trees"),
            Reply::new(201, json!({"sha": NEW_TREE}).to_string()),
        )
        .route(
            Method::Post,
            &url("/git/commits"),
            Reply::new(201, json!({"sha": COMMIT}).to_string()),
        )
        .route(
            Method::Post,
            &url("/pulls"),
            Reply::new(201, json!({"number": PR}).to_string()),
        );
    match branch {
        Branch::New => transport.route(
            Method::Post,
            &refs_url(),
            Reply::new(201, json!({"ref": "r"}).to_string()),
        ),
        Branch::Exists => transport
            .route(
                Method::Post,
                &refs_url(),
                Reply::new(422, r#"{"message":"Reference already exists"}"#),
            )
            .route(
                Method::Patch,
                &branch_ref_url(number),
                Reply::new(200, json!({"ref": "r"}).to_string()),
            ),
    }
}

/// Every request from the first one to `first` on, the token request left out.
pub fn calls_from(transport: &ScriptedTransport, first: &str) -> Vec<(Method, String)> {
    transport
        .requests()
        .into_iter()
        .filter(|request| request.url != TOKEN_URL)
        .skip_while(|request| request.url != first)
        .map(|request| (request.method, request.url))
        .collect()
}
