//! A scripted GitHub for `catalogue bot run`: the setup calls, a repo archive and a tier snapshot.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use catalogue_core::{Date, EntryKind, Tier};
use catalogue_github::testing::{Method, REDACTED, Recorded, Reply, ScriptedTransport};
use catalogue_signals::USER_AGENT;
use catalogue_signals::history::{Row, Snapshot, write};
use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Value, json};

/// The repo base URL.
pub const REPO: &str = "https://api.github.com/repos/ml-rust/catalogue-test";
/// The head of `main`.
pub const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
/// The App slug.
pub const SLUG: &str = "catalogue-bot";
/// The bot account login.
pub const BOT_LOGIN: &str = "catalogue-bot[bot]";
/// The installation token. It must never appear in a recorded request.
pub const TOKEN: &str = "ghs_run_secret";
/// The token request URL.
pub const TOKEN_URL: &str = "https://api.github.com/app/installations/77/access_tokens";

/// The labels file. The scripted repo already holds exactly these labels.
pub const LABELS: &str = r#"
[[label]]
name = "submit"
color = "0e8a16"
description = "Propose a new project for the catalogue"

[[label]]
name = "judgment:demotion"
color = "b60205"
description = "Approve moving a Featured entry to a lower tier"

[[label]]
name = "approved"
color = "2ea44f"
description = "A maintainer approves the judgment"

[[label]]
name = "bot-checked"
color = "c5def5"
description = "The bot checked the issue and found no error"

[[label]]
name = "needs-fix"
color = "e11d21"
description = "The bot check found errors to fix"

[[label]]
name = "pr-open"
color = "6f42c1"
description = "The bot opened the data PR for this issue"
"#;

/// The tier snapshots on the `signals` branch.
pub enum Signals<'a> {
    /// The branch does not exist.
    Missing,
    /// The newest snapshot holds this entry at this tier.
    Tier(&'a str, Tier),
}

/// `tail` under the repo URL.
pub fn url(tail: &str) -> String {
    format!("{REPO}{tail}")
}

/// The URL that lists the comments of issue `number`.
pub fn comments_url(number: u64) -> String {
    url(&format!("/issues/{number}/comments?per_page=100"))
}

/// The URL that posts a comment on issue `number`.
pub fn post_comment_url(number: u64) -> String {
    url(&format!("/issues/{number}/comments"))
}

/// The URL of comment `id`.
pub fn comment_url(id: u64) -> String {
    url(&format!("/issues/comments/{id}"))
}

/// The labels URL of issue `number`.
pub fn issue_labels_url(number: u64) -> String {
    url(&format!("/issues/{number}/labels"))
}

/// The issue body fixture `name` of the `catalogue-bot` crate.
pub fn body(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../catalogue-bot/tests/fixtures")
        .join(name);
    fs::read_to_string(path).expect("body fixture")
}

/// An open issue by `dave` as the listing returns it.
pub fn issue(number: u64, body: &str, labels: &[&str]) -> Value {
    issue_by(number, body, labels, "dave")
}

/// An open issue by `author` as the API returns it.
pub fn issue_by(number: u64, body: &str, labels: &[&str], author: &str) -> Value {
    let labels: Vec<Value> = labels
        .iter()
        .map(|name| json!({"name": name, "color": "000000"}))
        .collect();
    json!({
        "number": number,
        "state": "open",
        "title": "an issue",
        "body": body,
        "user": {"login": author, "type": "User"},
        "labels": labels,
    })
}

/// A comment on issue `number` by `login` of account type `kind`.
pub fn comment(id: u64, number: u64, login: &str, kind: &str, body: &str) -> Value {
    json!({
        "id": id,
        "body": body,
        "user": {"login": login, "type": kind},
        "issue_url": url(&format!("/issues/{number}")),
    })
}

/// A GitHub with the App, the labels, `main`, the `signals` branch and the open `issues`.
pub fn github(signals: &Signals<'_>, issues: &[Value]) -> ScriptedTransport {
    let label_list: Vec<Value> = catalogue_bot::parse_labels(LABELS)
        .expect("labels")
        .labels()
        .iter()
        .map(|label| {
            json!({"name": label.name, "color": label.color, "description": label.description})
        })
        .collect();
    let transport = ScriptedTransport::new()
        .route(
            Method::Post,
            TOKEN_URL,
            Reply::new(
                201,
                format!(r#"{{"token":"{TOKEN}","expires_at":"2026-11-04T01:00:00Z"}}"#),
            ),
        )
        .route(
            Method::Get,
            "https://api.github.com/app",
            Reply::new(200, format!(r#"{{"slug":"{SLUG}"}}"#)),
        )
        .route(
            Method::Get,
            &url("/labels?per_page=100"),
            Reply::new(200, Value::Array(label_list).to_string()),
        )
        .route(Method::Get, &url("/commits/main"), Reply::new(200, SHA))
        .route(
            Method::Get,
            &url(&format!("/tarball/{SHA}")),
            Reply::new(200, tarball()),
        )
        .route(
            Method::Get,
            &url("/issues?state=open&sort=updated&direction=asc&per_page=100"),
            Reply::new(200, Value::Array(issues.to_vec()).to_string()),
        );
    match signals {
        Signals::Missing => transport.route(
            Method::Get,
            &url("/git/trees/signals"),
            Reply::new(404, r#"{"message":"Not Found"}"#),
        ),
        Signals::Tier(slug, tier) => transport
            .route(
                Method::Get,
                &url("/git/trees/signals"),
                Reply::new(
                    200,
                    json!({"sha": "t", "truncated": false, "tree": [
                        {"path": "2026-10-01.jsonl.gz", "type": "blob"},
                        {"path": "2026-11-03.jsonl.gz", "type": "blob"},
                        {"path": "notes", "type": "tree"},
                    ]})
                    .to_string(),
                ),
            )
            .route(
                Method::Get,
                &url("/contents/2026-11-03.jsonl.gz?ref=signals"),
                Reply::new(200, snapshot(slug, *tier)),
            ),
    }
}

/// A gzip tar archive of the `valid` data fixture, as GitHub serves it for [`SHA`].
fn tarball() -> Vec<u8> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid");
    let mut builder = tar::Builder::new(Vec::new());
    builder
        .append_dir_all("ml-rust-catalogue-test-0123456/data", fixture)
        .expect("archive the fixture");
    builder
        .append_data(
            &mut header(6),
            "ml-rust-catalogue-test-0123456/README.md",
            &b"readme"[..],
        )
        .expect("archive the readme");
    let tar = builder.into_inner().expect("tar");
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    gz.write_all(&tar).expect("gzip");
    gz.finish().expect("gzip")
}

/// A regular file header of `size` bytes.
fn header(size: u64) -> tar::Header {
    let mut header = tar::Header::new_gnu();
    header.set_size(size);
    header.set_mode(0o644);
    header
}

/// The gzip bytes of a snapshot that holds `slug` at `tier`, written by the history writer.
fn snapshot(slug: &str, tier: Tier) -> Vec<u8> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    fs::create_dir_all(&base).expect("target dir");
    let dir = tempfile::tempdir_in(base).expect("scratch directory");
    let row = Row {
        kind: EntryKind::Library,
        tier,
        dead_now: false,
        dormant: None,
        active_users: None,
        docker_pulls: None,
        network: None,
        reused: false,
        fetch_notes: Vec::new(),
    };
    let snapshot = Snapshot {
        date: Date::new(2026, 11, 3).expect("date"),
        rows: [(slug.to_owned(), row)].into_iter().collect(),
    };
    let path: PathBuf = write(dir.path(), &snapshot).expect("snapshot");
    fs::read(path).expect("snapshot bytes")
}

/// `transport` with the routes of an issue that has no bot comment yet: the empty
/// listing, a post that creates comment `id` and the label write.
pub fn new_comment(transport: ScriptedTransport, number: u64, id: u64) -> ScriptedTransport {
    transport
        .route(Method::Get, &comments_url(number), Reply::new(200, "[]"))
        .route(
            Method::Post,
            &post_comment_url(number),
            Reply::new(201, json!({"id": id}).to_string()),
        )
        .route(
            Method::Post,
            &issue_labels_url(number),
            Reply::new(200, "[]"),
        )
}

/// Every request that writes to GitHub, the token request left out.
pub fn writes(transport: &ScriptedTransport) -> Vec<Recorded> {
    transport
        .requests()
        .into_iter()
        .filter(|request| request.method != Method::Get && request.url != TOKEN_URL)
        .collect()
}

/// The `body` field of a recorded comment write.
pub fn comment_text(request: &Recorded) -> String {
    request
        .json()
        .and_then(|value| value["body"].as_str().map(ToOwned::to_owned))
        .expect("comment body")
}

/// Asserts that every request sent the catalogue `User-Agent`, that every
/// GitHub request sent a redacted token, and that no other request sent one.
pub fn assert_redacted(transport: &ScriptedTransport) {
    assert_redacted_for(transport, &[], &[]);
}

/// Asserts what [`assert_redacted`] asserts, except that a request to a URL
/// under one of `keyed` also sends a redacted key. No request shows the
/// token or any of `secrets`.
pub fn assert_redacted_for(transport: &ScriptedTransport, keyed: &[&str], secrets: &[&str]) {
    let requests = transport.requests();
    assert!(!requests.is_empty());
    for request in requests {
        assert_eq!(request.header("User-Agent"), Some(USER_AGENT));
        let expected = std::iter::once("https://api.github.com/")
            .chain(keyed.iter().copied())
            .any(|prefix| request.url.starts_with(prefix))
            .then_some(REDACTED);
        assert_eq!(request.header("Authorization"), expected, "{}", request.url);
        let shown = format!("{request:?}");
        for secret in std::iter::once(TOKEN).chain(secrets.iter().copied()) {
            assert!(!shown.contains(secret), "{shown}");
        }
    }
}
