//! `catalogue bot run` over a scripted GitHub.

#[path = "common/bot_github.rs"]
mod bot_github;
#[path = "common/bot_pass.rs"]
mod bot_pass;
#[path = "common/bot_setup.rs"]
mod bot_setup;

use std::fs;

use bot_github::{
    BOT_LOGIN, LABELS, SHA, Signals, body, comment, comment_text, comment_url, comments_url,
    github, issue, issue_labels_url, new_comment, post_comment_url, writes,
};
use bot_pass::{run, state_dir, try_run};
use bot_setup::{BotSetup, bot_setup};
use catalogue::RunError;
use catalogue::bot::run::{LOCK_FILE, take_lock};
use catalogue_bot::{Marker, Sha256};
use catalogue_core::{CommitSha, Tier};
use catalogue_github::testing::{Method, Reply};
use serde_json::{Value, json};

/// The state file as JSON.
fn state(setup: &BotSetup) -> Value {
    let text = fs::read_to_string(state_dir(setup).join("bot-state.json")).expect("state file");
    serde_json::from_str(&text).expect("state json")
}

/// The submission body, edited when `edited`.
fn submit_body(edited: bool) -> String {
    let text = body("submit.md");
    if edited {
        text.replace("A gizmo that learns", "A gizmo that trains")
    } else {
        text
    }
}

/// A first run on issue 12 that posts comment 900. Returns the posted text.
fn first_run(setup: &BotSetup) -> String {
    let transport = new_comment(
        github(
            &Signals::Missing,
            &[issue(12, &submit_body(false), &["submit"])],
        ),
        12,
        900,
    );
    let report = run(setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let posts: Vec<_> = writes(&transport)
        .into_iter()
        .filter(|request| request.url == post_comment_url(12))
        .collect();
    assert_eq!(posts.len(), 1);
    comment_text(&posts[0])
}

/// The bot comment 900 on issue 12 with `text`.
fn bot_comment(text: &str) -> Reply {
    Reply::new(200, comment(900, 12, BOT_LOGIN, "Bot", text).to_string())
}

#[test]
fn a_first_run_posts_the_comment_and_the_label() {
    let setup = bot_setup(LABELS);
    let transport = new_comment(
        github(
            &Signals::Missing,
            &[issue(12, &submit_body(false), &["submit"])],
        ),
        12,
        900,
    );
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        report.lines,
        ["issue 12: comment posted, added bot-checked"]
    );

    let written = writes(&transport);
    assert_eq!(written.len(), 2, "{written:?}");
    let text = comment_text(&written[0]);
    let marker = Marker::of_comment(&text).expect("marker");
    assert_eq!(marker.body, Sha256::of(submit_body(false).as_bytes()));
    assert_eq!(marker.main, Some(CommitSha::new(SHA).expect("sha")));
    assert!(text.contains("**Result: no blocking error.**"), "{text}");
    assert_eq!(written[1].url, issue_labels_url(12));
    assert_eq!(written[1].json(), Some(json!({"labels": ["bot-checked"]})));

    let saved = state(&setup);
    assert_eq!(saved["version"], 1);
    assert_eq!(saved["issues"]["12"]["comment_id"], 900);
    assert_eq!(saved["issues"]["12"]["main"], SHA);
    assert_eq!(
        saved["issues"]["12"]["body_sha256"],
        marker.body.to_string()
    );
}

#[test]
fn an_identical_second_run_writes_nothing() {
    let setup = bot_setup(LABELS);
    let posted = first_run(&setup);
    let before = state(&setup);
    let transport = github(
        &Signals::Missing,
        &[issue(12, &submit_body(false), &["submit", "bot-checked"])],
    )
    .route(Method::Get, &comment_url(900), bot_comment(&posted));
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(report.lines, ["issue 12: up to date"]);
    assert!(writes(&transport).is_empty(), "{:?}", writes(&transport));
    assert_eq!(state(&setup), before);
}

#[test]
fn an_edited_body_updates_the_comment() {
    let setup = bot_setup(LABELS);
    let posted = first_run(&setup);
    let transport = github(
        &Signals::Missing,
        &[issue(12, &submit_body(true), &["submit", "bot-checked"])],
    )
    .route(Method::Get, &comment_url(900), bot_comment(&posted))
    .route(Method::Patch, &comment_url(900), Reply::new(200, "{}"));
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let written = writes(&transport);
    assert_eq!(written.len(), 1, "{written:?}");
    assert_eq!(written[0].method, Method::Patch);
    let marker = Marker::of_comment(&comment_text(&written[0])).expect("marker");
    assert_eq!(marker.body, Sha256::of(submit_body(true).as_bytes()));
    assert!(comment_text(&written[0]).contains("A gizmo that trains"));
}

#[test]
fn a_removed_label_is_added_again() {
    let setup = bot_setup(LABELS);
    let posted = first_run(&setup);
    let transport = github(
        &Signals::Missing,
        &[issue(12, &submit_body(false), &["submit"])],
    )
    .route(Method::Get, &comment_url(900), bot_comment(&posted))
    .route(Method::Post, &issue_labels_url(12), Reply::new(200, "[]"));
    run(&setup, &transport);
    let written = writes(&transport);
    assert_eq!(written.len(), 1, "{written:?}");
    assert_eq!(written[0].json(), Some(json!({"labels": ["bot-checked"]})));
}

#[test]
fn a_closed_issue_leaves_the_state() {
    let setup = bot_setup(LABELS);
    first_run(&setup);
    let transport = github(&Signals::Missing, &[]);
    let report = run(&setup, &transport);
    assert!(report.lines.is_empty());
    assert!(writes(&transport).is_empty());
    assert_eq!(state(&setup)["issues"], json!({}));
}

#[test]
fn a_deleted_comment_is_posted_again() {
    let setup = bot_setup(LABELS);
    let posted = first_run(&setup);
    let transport = github(
        &Signals::Missing,
        &[issue(12, &submit_body(true), &["submit", "bot-checked"])],
    )
    .route(Method::Get, &comment_url(900), bot_comment(&posted))
    .route(Method::Patch, &comment_url(900), Reply::new(404, "{}"))
    .route(Method::Get, &comments_url(12), Reply::new(200, "[]"))
    .route(
        Method::Post,
        &post_comment_url(12),
        Reply::new(201, r#"{"id":901}"#),
    );
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let methods: Vec<Method> = writes(&transport).iter().map(|r| r.method).collect();
    assert_eq!(methods, [Method::Patch, Method::Post]);
    assert_eq!(state(&setup)["issues"]["12"]["comment_id"], 901);
}

#[test]
fn approval_of_a_changed_body_is_removed() {
    let setup = bot_setup(LABELS);
    let posted = first_run(&setup);
    let transport = github(
        &Signals::Missing,
        &[issue(
            12,
            &submit_body(true),
            &["submit", "bot-checked", "approved"],
        )],
    )
    .route(Method::Get, &comment_url(900), bot_comment(&posted))
    .route(
        Method::Delete,
        &format!("{}/approved", issue_labels_url(12)),
        Reply::new(200, "[]"),
    )
    .route(Method::Patch, &comment_url(900), Reply::new(200, "{}"));
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let written = writes(&transport);
    let methods: Vec<Method> = written.iter().map(|r| r.method).collect();
    assert_eq!(methods, [Method::Delete, Method::Patch]);
    let text = comment_text(&written[1]);
    assert!(text.contains("**Approval removed.**"), "{text}");
    assert!(Marker::of_comment(&text).is_some());
}

#[test]
fn a_forged_marker_by_another_account_is_ignored() {
    let setup = bot_setup(LABELS);
    let forged = Marker {
        body: Sha256::of(submit_body(false).as_bytes()),
        report: Sha256::of(b"anything"),
        main: None,
    }
    .render();
    let comments = json!([
        comment(5, 12, "mallory", "User", &forged),
        comment(6, 12, BOT_LOGIN, "User", &forged),
        comment(7, 12, "other-bot[bot]", "Bot", &forged),
    ]);
    let transport = github(
        &Signals::Missing,
        &[issue(12, &submit_body(false), &["submit"])],
    )
    .route(
        Method::Get,
        &comments_url(12),
        Reply::new(200, comments.to_string()),
    )
    .route(
        Method::Post,
        &post_comment_url(12),
        Reply::new(201, r#"{"id":900}"#),
    )
    .route(Method::Post, &issue_labels_url(12), Reply::new(200, "[]"));
    run(&setup, &transport);
    let written = writes(&transport);
    assert!(
        written.iter().all(|r| r.method == Method::Post),
        "{written:?}"
    );
    assert_eq!(written[0].url, post_comment_url(12));
}

#[test]
fn two_form_labels_give_a_blocking_report() {
    let setup = bot_setup(LABELS);
    let transport = new_comment(
        github(
            &Signals::Missing,
            &[issue(
                12,
                &submit_body(false),
                &["submit", "judgment:demotion"],
            )],
        ),
        12,
        900,
    );
    run(&setup, &transport);
    let written = writes(&transport);
    let text = comment_text(&written[0]);
    assert!(text.contains("**Result: blocking.**"), "{text}");
    assert!(text.contains("exactly one form label"), "{text}");
    assert_eq!(written[1].json(), Some(json!({"labels": ["needs-fix"]})));
}

#[test]
fn a_held_lock_refuses_the_run() {
    let setup = bot_setup(LABELS);
    let _held = take_lock(&state_dir(&setup)).expect("lock");
    let transport = github(&Signals::Missing, &[]);
    let error = try_run(&setup, &transport).expect_err("locked");
    assert!(matches!(error, RunError::Locked { .. }), "{error}");
    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains(LOCK_FILE), "{error}");
    assert!(transport.requests().is_empty());
}

/// The comment a run posts on demotion issue 13 with the `signals` branch as given.
fn demotion_comment(signals: &Signals<'_>) -> String {
    let setup = bot_setup(LABELS);
    let transport = new_comment(
        github(
            signals,
            &[issue(13, &body("demotion.md"), &["judgment:demotion"])],
        ),
        13,
        910,
    );
    run(&setup, &transport);
    comment_text(&writes(&transport)[0])
}

#[test]
fn a_demotion_reads_the_tier_from_the_newest_snapshot() {
    let featured = demotion_comment(&Signals::Tier("alpha", Tier::Featured));
    assert!(
        featured.contains("The entry is `featured` now. The demotion can go ahead."),
        "{featured}"
    );
    let listed = demotion_comment(&Signals::Tier("alpha", Tier::Listed));
    assert!(listed.contains("The entry is `listed` now."), "{listed}");
    let missing = demotion_comment(&Signals::Missing);
    assert!(missing.contains("Current tier unknown."), "{missing}");
}
