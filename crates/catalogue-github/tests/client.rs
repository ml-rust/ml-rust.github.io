//! The App client and the repo client over a scripted transport.

#[path = "common/key_files.rs"]
mod key_files;

use std::cell::Cell;

use catalogue_github::testing::{Method, NoWait, REDACTED, Reply, ScriptedTransport};
use catalogue_github::{Access, ApiError, AppKey, GithubApp, LabelFields, MAX_PAGES, Permissions};
use catalogue_signals::USER_AGENT;
use catalogue_signals::github::RepoRef;
use jiff::Timestamp;
use key_files::{key_copy, scratch};

const TOKEN_URL: &str = "https://api.github.com/app/installations/77/access_tokens";
const LABELS_URL: &str = "https://api.github.com/repos/ml-rust/catalogue-test/labels";
const LIST_URL: &str = "https://api.github.com/repos/ml-rust/catalogue-test/labels?per_page=100";

fn at(text: &str) -> Timestamp {
    text.parse().expect("time")
}

fn key() -> AppKey {
    let dir = scratch();
    AppKey::read(&key_copy(dir.path(), 0o600)).expect("key")
}

fn token_reply(token: &str, expires_at: &str) -> Reply {
    Reply::new(
        201,
        format!(r#"{{"token":"{token}","expires_at":"{expires_at}","permissions":{{}}}}"#),
    )
}

fn permissions() -> Permissions {
    Permissions::new()
        .with("issues", Access::Write)
        .with("contents", Access::Read)
}

fn repo() -> RepoRef {
    RepoRef::parse("ml-rust/catalogue-test").expect("repo")
}

/// Runs `body` against a repo client over `transport` at the clock in `now`.
fn with_client<R>(
    transport: &ScriptedTransport,
    now: &Cell<Timestamp>,
    body: impl FnOnce(&mut catalogue_github::RepoClient<'_>) -> R,
) -> R {
    let mut pacer = NoWait::default();
    let clock = || now.get();
    let app = GithubApp::new(transport, &mut pacer, 123_456, key(), &clock);
    let mut client = app.repo(77, repo(), permissions());
    body(&mut client)
}

#[test]
fn every_request_sends_the_pinned_headers_and_no_personal_identifier() {
    let transport = ScriptedTransport::new()
        .route(
            Method::Post,
            TOKEN_URL,
            token_reply("ghs_one", "2026-11-04T01:00:00Z"),
        )
        .route(Method::Get, LIST_URL, Reply::new(200, "[]"))
        .route(
            Method::Get,
            "https://api.github.com/app",
            Reply::new(200, r#"{"slug":"catalogue-bot","id":1}"#),
        );
    let now = Cell::new(at("2026-11-04T00:00:00Z"));
    let slug = with_client(&transport, &now, |client| {
        client.labels().expect("labels");
        client.app().slug().expect("slug")
    });
    assert_eq!(slug, "catalogue-bot");
    let requests = transport.requests();
    assert_eq!(requests.len(), 3);
    for request in &requests {
        assert_eq!(request.header("User-Agent"), Some(USER_AGENT));
        assert_eq!(
            request.header("Accept"),
            Some("application/vnd.github+json")
        );
        assert_eq!(request.header("X-GitHub-Api-Version"), Some("2022-11-28"));
        assert_eq!(request.header("Authorization"), Some(REDACTED));
    }
    let token = &requests[0];
    assert_eq!(token.method, Method::Post);
    assert_eq!(token.header("Content-Type"), Some("application/json"));
    assert_eq!(
        token.json().expect("json body"),
        serde_json::json!({
            "repositories": ["catalogue-test"],
            "permissions": {"issues": "write", "contents": "read"},
        })
    );
}

#[test]
fn the_token_is_reused_while_fresh_and_minted_again_near_expiry() {
    let transport = ScriptedTransport::new()
        .route(
            Method::Post,
            TOKEN_URL,
            token_reply("ghs_one", "2026-11-04T01:00:00Z"),
        )
        .route(
            Method::Post,
            TOKEN_URL,
            token_reply("ghs_two", "2026-11-04T02:00:00Z"),
        )
        .route(Method::Get, LIST_URL, Reply::new(200, "[]"));
    let now = Cell::new(at("2026-11-04T00:00:00Z"));
    with_client(&transport, &now, |client| {
        client.labels().expect("first");
        now.set(at("2026-11-04T00:55:00Z"));
        client.labels().expect("still fresh");
        now.set(at("2026-11-04T00:55:01Z"));
        client.labels().expect("refreshed");
        now.set(at("2026-11-04T01:30:00Z"));
        client.labels().expect("fresh again");
    });
    assert_eq!(transport.requests_with(Method::Post).len(), 2);
    assert_eq!(transport.requests_with(Method::Get).len(), 4);
}

#[test]
fn a_listing_follows_next_links_across_pages() {
    let page2 = format!("{LIST_URL}&page=2");
    let transport = ScriptedTransport::new()
        .route(
            Method::Post,
            TOKEN_URL,
            token_reply("t", "2026-11-04T01:00:00Z"),
        )
        .route(
            Method::Get,
            LIST_URL,
            Reply::new(
                200,
                r#"[{"name":"submit","color":"0e8a16","description":null}]"#,
            )
            .header(
                "Link",
                &format!("<{page2}>; rel=\"next\", <{page2}>; rel=\"last\""),
            ),
        )
        .route(
            Method::Get,
            &page2,
            Reply::new(
                200,
                r#"[{"name":"bug","color":"d73a4a","description":"Broken"}]"#,
            )
            .header("Link", &format!("<{LIST_URL}>; rel=\"prev\"")),
        );
    let now = Cell::new(at("2026-11-04T00:00:00Z"));
    let labels = with_client(&transport, &now, |client| client.labels().expect("labels"));
    let names: Vec<&str> = labels.iter().map(|label| label.name.as_str()).collect();
    assert_eq!(names, ["submit", "bug"]);
    assert_eq!(labels[1].description.as_deref(), Some("Broken"));
}

#[test]
fn a_listing_past_the_page_cap_is_an_error() {
    let mut transport = ScriptedTransport::new().route(
        Method::Post,
        TOKEN_URL,
        token_reply("t", "2026-11-04T01:00:00Z"),
    );
    let page = |n: usize| {
        if n == 1 {
            LIST_URL.to_owned()
        } else {
            format!("{LIST_URL}&page={n}")
        }
    };
    for n in 1..=MAX_PAGES + 1 {
        let next = page(n + 1);
        transport = transport.route(
            Method::Get,
            &page(n),
            Reply::new(200, "[]").header("Link", &format!("<{next}>; rel=\"next\"")),
        );
    }
    let now = Cell::new(at("2026-11-04T00:00:00Z"));
    let error = with_client(&transport, &now, |client| {
        client.labels().expect_err("capped")
    });
    assert!(
        matches!(error, ApiError::TooManyPages { pages, .. } if pages == MAX_PAGES),
        "{error}"
    );
    assert_eq!(transport.requests_with(Method::Get).len(), MAX_PAGES);
}

#[test]
fn a_next_link_off_the_api_host_is_not_followed() {
    let transport = ScriptedTransport::new()
        .route(
            Method::Post,
            TOKEN_URL,
            token_reply("t", "2026-11-04T01:00:00Z"),
        )
        .route(
            Method::Get,
            LIST_URL,
            Reply::new(200, "[]").header("Link", "<https://evil.example/x>; rel=\"next\""),
        );
    let now = Cell::new(at("2026-11-04T00:00:00Z"));
    let error = with_client(&transport, &now, |client| {
        client.labels().expect_err("refused")
    });
    assert!(matches!(error, ApiError::ForeignLink { .. }), "{error}");
    assert_eq!(transport.requests_with(Method::Get).len(), 1);
}

#[test]
fn create_and_update_send_the_label_fields() {
    let transport = ScriptedTransport::new()
        .route(
            Method::Post,
            TOKEN_URL,
            token_reply("t", "2026-11-04T01:00:00Z"),
        )
        .route(Method::Post, LABELS_URL, Reply::new(201, "{}"))
        .route(
            Method::Patch,
            &format!("{LABELS_URL}/Judgment%3Akind"),
            Reply::new(200, "{}"),
        );
    let fields = LabelFields {
        name: "judgment:kind".to_owned(),
        color: "1d76db".to_owned(),
        description: "Set an entry as a product or a demo".to_owned(),
    };
    let now = Cell::new(at("2026-11-04T00:00:00Z"));
    with_client(&transport, &now, |client| {
        client.create_label(&fields).expect("created");
        client
            .update_label("Judgment:kind", &fields)
            .expect("updated");
    });
    let posts = transport.requests_with(Method::Post);
    assert_eq!(posts[1].url, LABELS_URL);
    assert_eq!(
        posts[1].json().expect("json"),
        serde_json::json!({"name": "judgment:kind", "color": "1d76db", "description": "Set an entry as a product or a demo"})
    );
    let patches = transport.requests_with(Method::Patch);
    assert_eq!(patches.len(), 1);
    assert_eq!(
        patches[0].json().expect("json")["new_name"],
        "judgment:kind"
    );
}

#[test]
fn a_rejected_create_names_the_method_url_and_status() {
    let transport = ScriptedTransport::new()
        .route(
            Method::Post,
            TOKEN_URL,
            token_reply("ghs_secret_value", "2026-11-04T01:00:00Z"),
        )
        .route(Method::Post, LABELS_URL, Reply::new(422, "{}"));
    let fields = LabelFields {
        name: "submit".to_owned(),
        color: "0e8a16".to_owned(),
        description: String::new(),
    };
    let now = Cell::new(at("2026-11-04T00:00:00Z"));
    let error = with_client(&transport, &now, |client| {
        client.create_label(&fields).expect_err("rejected")
    });
    let message = error.to_string();
    assert!(
        message.starts_with(&format!("POST {LABELS_URL} answered with status 422.")),
        "{message}"
    );
    assert!(!message.contains("ghs_secret_value"), "{message}");
    assert!(!format!("{error:?}").contains("ghs_secret_value"));
}

#[test]
fn a_rejected_token_request_is_an_error_with_the_next_action() {
    let transport = ScriptedTransport::new().route(Method::Post, TOKEN_URL, Reply::new(401, "{}"));
    let now = Cell::new(at("2026-11-04T00:00:00Z"));
    let error = with_client(&transport, &now, |client| {
        client.labels().expect_err("unauthorized")
    });
    let message = error.to_string();
    assert!(message.contains("401"), "{message}");
    assert!(message.contains("system clock"), "{message}");
    assert!(transport.requests_with(Method::Get).is_empty());
}
