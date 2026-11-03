//! `catalogue bot labels` over a scripted GitHub.

#[path = "common/bot_setup.rs"]
mod bot_setup;

use bot_setup::{BotSetup, bot_setup};
use catalogue::{LabelsCommandError, LabelsReport, sync_labels};
use catalogue_github::testing::{Method, NoWait, REDACTED, Reply, ScriptedTransport};
use catalogue_signals::USER_AGENT;
use jiff::Timestamp;
use serde_json::json;

const TOKEN_URL: &str = "https://api.github.com/app/installations/77/access_tokens";
const LABELS_URL: &str = "https://api.github.com/repos/ml-rust/catalogue-test/labels";
const LIST_URL: &str = "https://api.github.com/repos/ml-rust/catalogue-test/labels?per_page=100";

const LABELS: &str = r#"
[[label]]
name = "submit"
color = "0e8a16"
description = "Propose a new project for the catalogue"

[[label]]
name = "judgment:kind"
color = "1d76db"
description = "Set an entry as a product or a demo"

[[label]]
name = "needs-fix"
color = "e11d21"
description = "The bot check found errors to fix"
"#;

/// A repo with `submit` in line, `Judgment:Kind` out of date, `bug` unknown
/// and `needs-fix` missing, over two pages.
fn github() -> ScriptedTransport {
    let page2 = format!("{LIST_URL}&page=2");
    ScriptedTransport::new()
        .route(
            Method::Post,
            TOKEN_URL,
            Reply::new(
                201,
                r#"{"token":"ghs_cli_secret","expires_at":"2026-11-04T01:00:00Z"}"#,
            ),
        )
        .route(
            Method::Get,
            LIST_URL,
            Reply::new(
                200,
                r#"[{"name":"bug","color":"d73a4a","description":"Something is broken"},
                    {"name":"submit","color":"0E8A16","description":"Propose a new project for the catalogue"}]"#,
            )
            .header("Link", &format!("<{page2}>; rel=\"next\"")),
        )
        .route(
            Method::Get,
            &page2,
            Reply::new(
                200,
                r#"[{"name":"Judgment:Kind","color":"1d76db","description":null}]"#,
            ),
        )
        .route(Method::Post, LABELS_URL, Reply::new(201, "{}"))
        .route(
            Method::Patch,
            &format!("{LABELS_URL}/Judgment%3AKind"),
            Reply::new(200, "{}"),
        )
}

fn now() -> Timestamp {
    "2026-11-04T00:00:00Z".parse().expect("time")
}

fn sync(
    setup: &BotSetup,
    transport: &ScriptedTransport,
    pacer: &mut NoWait,
    dry_run: bool,
) -> Result<LabelsReport, LabelsCommandError> {
    sync_labels(
        transport,
        pacer,
        &now,
        &setup.config,
        &setup.labels,
        dry_run,
    )
}

const PLAN: &str = "create needs-fix: color e11d21, description \"The bot check found errors to fix\"\n\
update Judgment:Kind: name -> judgment:kind; description \"\" -> \"Set an entry as a product or a demo\"\n\
unknown bug: not in the labels file, left in place\n";

#[test]
fn a_dry_run_plans_and_writes_nothing() {
    let setup = bot_setup(LABELS);
    let transport = github();
    let mut pacer = NoWait::default();
    let report = sync(&setup, &transport, &mut pacer, true).expect("plans");
    assert_eq!(
        report.render(),
        format!("{PLAN}dry run: 2 change(s) not applied\n")
    );
    assert!(transport.requests_with(Method::Patch).is_empty());
    let posts = transport.requests_with(Method::Post);
    assert_eq!(posts.len(), 1, "only the token request");
    assert_eq!(posts[0].url, TOKEN_URL);
}

#[test]
fn a_sync_creates_and_updates_and_never_deletes() {
    let setup = bot_setup(LABELS);
    let transport = github();
    let mut pacer = NoWait::default();
    let report = sync(&setup, &transport, &mut pacer, false).expect("syncs");
    assert_eq!(
        report.render(),
        format!("{PLAN}applied 1 create(s) and 1 update(s)\n")
    );

    let posts = transport.requests_with(Method::Post);
    assert_eq!(posts.len(), 2);
    assert_eq!(
        posts[0].json().expect("token body"),
        json!({
            "repositories": ["catalogue-test"],
            "permissions": {"issues": "write", "contents": "read"},
        })
    );
    assert_eq!(posts[1].url, LABELS_URL);
    assert_eq!(
        posts[1].json().expect("create body"),
        json!({"name": "needs-fix", "color": "e11d21", "description": "The bot check found errors to fix"})
    );
    let patches = transport.requests_with(Method::Patch);
    assert_eq!(patches.len(), 1);
    assert_eq!(
        patches[0].json().expect("update body"),
        json!({"new_name": "judgment:kind", "color": "1d76db", "description": "Set an entry as a product or a demo"})
    );
    assert!(transport.requests_with(Method::Delete).is_empty());

    for request in transport.requests() {
        assert_eq!(request.header("User-Agent"), Some(USER_AGENT));
        assert_eq!(request.header("Authorization"), Some(REDACTED));
        assert!(!format!("{request:?}").contains("ghs_cli_secret"));
    }
    assert!(pacer.waits >= transport.requests().len() - 1);
}

#[test]
fn a_bad_labels_file_stops_before_any_request() {
    let setup = bot_setup("[[label]]\nname = \"a\"\ncolor = \"#000000\"\ndescription = \"\"\n");
    let transport = github();
    let mut pacer = NoWait::default();
    let error = sync(&setup, &transport, &mut pacer, false).expect_err("refused");
    assert!(
        matches!(error, LabelsCommandError::LabelsFile { .. }),
        "{error}"
    );
    assert_eq!(error.exit_code(), 1);
    assert!(error.to_string().contains("color `#000000`"), "{error}");
    assert!(transport.requests().is_empty());
}

#[cfg(unix)]
#[test]
fn an_exposed_key_stops_before_any_request() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let setup = bot_setup(LABELS);
    fs::set_permissions(
        &setup.config.private_key_path,
        fs::Permissions::from_mode(0o644),
    )
    .expect("chmod");
    let transport = github();
    let mut pacer = NoWait::default();
    let error = sync(&setup, &transport, &mut pacer, false).expect_err("refused");
    assert!(error.to_string().contains("chmod 600"), "{error}");
    assert_eq!(error.exit_code(), 1);
    assert!(transport.requests().is_empty());
}

#[test]
fn a_failed_call_exits_two() {
    let setup = bot_setup(LABELS);
    let transport = ScriptedTransport::new().route(Method::Post, TOKEN_URL, Reply::new(401, "{}"));
    let mut pacer = NoWait::default();
    let error = sync(&setup, &transport, &mut pacer, true).expect_err("unauthorized");
    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("status 401"), "{error}");
}
