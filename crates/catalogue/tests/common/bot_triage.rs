//! The link check config, its key variables, and scripted model and `web_fetch` routes.

use catalogue_github::testing::{Method, Recorded, Reply, ScriptedTransport};
use catalogue_triage::WEB_FETCH_URL;
use jiff::Timestamp;
use serde_json::json;
use url::Url;

use super::bot_github::{BOT_LOGIN, assert_redacted_for, comment, writes};
use super::bot_pass::try_run_at;
use super::bot_setup::BotSetup;

/// The chat endpoint of the first provider.
pub const OLLAMA_CHAT: &str = "https://ollama.com/v1/chat/completions";

/// The chat endpoint of the second provider.
pub const OPENROUTER_CHAT: &str = "https://openrouter.ai/api/v1/chat/completions";

/// The evidence link of the submission fixture.
pub const EVIDENCE: &str = "https://example.com/uses-gizmo";

/// The page text the fetch service returns. The model quotes it.
pub const PAGE: &str = "Acme runs gizmo-core in production.";

/// The seconds a failed check waits.
pub const RETRY_AFTER: i64 = 600;

/// The `[triage]` section of the test config: two providers and the fetch service.
pub const TRIAGE: &str = r#"
[triage]
retry_after_secs = 600

[[triage.providers]]
name = "ollama"
base_url = "https://ollama.com/v1"
api_key_env = "TEST_OLLAMA_KEY"
models = ["m-1"]

[[triage.providers]]
name = "openrouter"
base_url = "https://openrouter.ai/api/v1"
api_key_env = "TEST_OPENROUTER_KEY"
models = ["r-1"]

[triage.fetch]
api_key_env = "TEST_WEB_KEY"
"#;

/// The key variables. No request, warning or summary shows a value.
pub const KEYS: [(&str, &str); 3] = [
    ("TEST_OLLAMA_KEY", "sk-ollama-secret"),
    ("TEST_OPENROUTER_KEY", "sk-router-secret"),
    ("TEST_WEB_KEY", "web-secret"),
];

/// `transport` with a fetch service that returns `content` for any link.
pub fn with_page(transport: ScriptedTransport, content: &str) -> ScriptedTransport {
    transport.route(
        Method::Post,
        WEB_FETCH_URL,
        Reply::new(
            200,
            json!({"title": "Gizmo users", "content": content, "links": []}).to_string(),
        ),
    )
}

/// `transport` with a first provider that backs the claim with `quote`.
pub fn answering(transport: ScriptedTransport, quote: &str) -> ScriptedTransport {
    let content = json!({"verdict": "supports", "quote": quote}).to_string();
    transport.route(
        Method::Post,
        OLLAMA_CHAT,
        Reply::new(
            200,
            json!({"choices": [{"message": {"role": "assistant", "content": content}}]})
                .to_string(),
        ),
    )
}

/// `transport` with both providers answering 500.
pub fn failing(transport: ScriptedTransport) -> ScriptedTransport {
    transport
        .route(Method::Post, OLLAMA_CHAT, Reply::new(500, "down"))
        .route(Method::Post, OPENROUTER_CHAT, Reply::new(500, "down"))
}

/// The model requests so far.
pub fn model_calls(transport: &ScriptedTransport) -> usize {
    transport
        .requests()
        .iter()
        .filter(|request| request.url == OLLAMA_CHAT || request.url == OPENROUTER_CHAT)
        .count()
}

/// The `web_fetch` requests so far.
pub fn fetch_calls(transport: &ScriptedTransport) -> Vec<Recorded> {
    transport
        .requests()
        .into_iter()
        .filter(|request| request.url == WEB_FETCH_URL)
        .collect()
}

/// The writes to GitHub so far, the token request left out.
pub fn github_writes(transport: &ScriptedTransport) -> Vec<Recorded> {
    writes(transport)
        .into_iter()
        .filter(|request| request.url.starts_with("https://api.github.com/"))
        .collect()
}

/// The bot comment 900 on issue 12 with `text`.
pub fn bot_comment(text: &str) -> Reply {
    Reply::new(200, comment(900, 12, BOT_LOGIN, "Bot", text).to_string())
}

/// `seconds` after the default test clock.
pub fn later(seconds: i64) -> Timestamp {
    let base: Timestamp = "2026-11-04T00:00:00Z".parse().expect("time");
    Timestamp::from_second(base.as_second() + seconds).expect("time")
}

/// One pass at `at` with the variables in `env` that must succeed.
///
/// Every model and fetch request carries a redacted key. No request shows a
/// key value or goes to the evidence host. No warning or summary line shows a
/// key value.
pub fn run_at(
    setup: &BotSetup,
    transport: &ScriptedTransport,
    env: &[(&str, &str)],
    at: Timestamp,
) -> catalogue::RunReport {
    let report = try_run_at(setup, transport, env, at).expect("runs");
    let secrets: Vec<&str> = KEYS.iter().map(|(_, value)| *value).collect();
    assert_redacted_for(
        transport,
        &[OLLAMA_CHAT, OPENROUTER_CHAT, WEB_FETCH_URL],
        &secrets,
    );
    let evidence = Url::parse(EVIDENCE).expect("link");
    for request in transport.requests() {
        let host = Url::parse(&request.url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned));
        assert_ne!(host.as_deref(), evidence.host_str(), "{}", request.url);
    }
    for line in report.lines.iter().chain(&report.warnings) {
        assert!(
            secrets.iter().all(|secret| !line.contains(secret)),
            "{line}"
        );
    }
    report
}
