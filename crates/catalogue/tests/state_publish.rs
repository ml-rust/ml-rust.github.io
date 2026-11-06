//! `catalogue state --publish` over a scripted GitHub.

#[path = "common/bot_setup.rs"]
mod bot_setup;

use bot_setup::bot_setup;
use catalogue::state::{STATE_PATH, publish_state};
use catalogue_github::testing::{Method, NoWait, Reply, ScriptedTransport};
use catalogue_github::{Access, ApiError, AppKey, GithubApp, Permissions};
use jiff::Timestamp;

const REPO: &str = "https://api.github.com/repos/ml-rust/catalogue-test";
const HEAD: &str = "1111111111111111111111111111111111111111";
const BASE: &str = "2222222222222222222222222222222222222222";
const TREE: &str = "3333333333333333333333333333333333333333";
const COMMIT: &str = "4444444444444444444444444444444444444444";

fn now() -> Timestamp {
    "2026-11-04T00:00:00Z".parse().expect("time")
}

/// A `signals` branch at [`HEAD`]. A new tree gets the id `tree`. Moving the branch answers `moved`.
fn github(tree: &str, moved: u16) -> ScriptedTransport {
    ScriptedTransport::new()
        .route(
            Method::Post,
            "https://api.github.com/app/installations/77/access_tokens",
            Reply::new(
                201,
                r#"{"token":"ghs_cli_secret","expires_at":"2026-11-04T01:00:00Z"}"#,
            ),
        )
        .route(
            Method::Get,
            &format!("{REPO}/commits/signals"),
            Reply::new(200, HEAD),
        )
        .route(
            Method::Get,
            &format!("{REPO}/git/commits/{HEAD}"),
            Reply::new(200, format!(r#"{{"tree":{{"sha":"{BASE}"}}}}"#)),
        )
        .route(
            Method::Post,
            &format!("{REPO}/git/trees"),
            Reply::new(201, format!(r#"{{"sha":"{tree}"}}"#)),
        )
        .route(
            Method::Post,
            &format!("{REPO}/git/commits"),
            Reply::new(201, format!(r#"{{"sha":"{COMMIT}"}}"#)),
        )
        .route(
            Method::Patch,
            &format!("{REPO}/git/refs/heads/signals"),
            Reply::new(moved, "{}"),
        )
}

fn publish(transport: &ScriptedTransport) -> Result<bool, ApiError> {
    let setup = bot_setup("");
    assert!(setup.labels.is_file(), "the setup is complete");
    let key = AppKey::read(&setup.config.private_key_path).expect("key");
    let mut pacer = NoWait::default();
    let clock = now;
    let app = GithubApp::new(
        transport,
        &mut pacer,
        setup.config.app_id.get(),
        key,
        &clock,
    );
    let mut client = app.repo(
        setup.config.installation_id.get(),
        setup.config.repo.clone(),
        Permissions::new().with("contents", Access::Write),
    );
    publish_state(&mut client, "{}\n")
}

#[test]
fn a_new_report_moves_the_branch_forward_and_never_forces_it() {
    let transport = github(TREE, 200);
    assert!(publish(&transport).expect("publishes"));
    let trees = transport.requests_with(Method::Post);
    let tree = trees
        .iter()
        .find(|request| request.url.ends_with("/git/trees"))
        .expect("tree");
    let body = String::from_utf8_lossy(&tree.body);
    assert!(body.contains(STATE_PATH), "{body}");
    assert!(body.contains(BASE), "the tree keeps the snapshots: {body}");
    let moves = transport.requests_with(Method::Patch);
    assert_eq!(moves.len(), 1);
    let body = String::from_utf8_lossy(&moves[0].body);
    assert!(body.contains(COMMIT), "{body}");
    assert!(body.contains(r#""force":false"#), "{body}");
}

#[test]
fn the_same_report_writes_no_commit() {
    let transport = github(BASE, 200);
    assert!(!publish(&transport).expect("publishes"));
    assert!(transport.requests_with(Method::Patch).is_empty());
}

#[test]
fn a_branch_that_keeps_moving_ends_in_an_error_after_each_try() {
    let transport = github(TREE, 422);
    let error = publish(&transport).expect_err("keeps moving");
    assert!(error.to_string().contains("Rerun later"), "{error}");
    assert_eq!(transport.requests_with(Method::Patch).len(), 5);
}
