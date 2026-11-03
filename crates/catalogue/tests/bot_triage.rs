//! The advisory LLM link check inside `catalogue bot run`, over a scripted GitHub,
//! scripted model providers and a scripted fetch service.

#[path = "common/bot_github.rs"]
mod bot_github;
#[path = "common/bot_pass.rs"]
mod bot_pass;
#[path = "common/bot_setup.rs"]
mod bot_setup;
#[path = "common/bot_triage.rs"]
mod bot_triage;

use std::fs;
use std::path::Path;

use bot_github::{
    LABELS, Signals, body, comment_text, comment_url, github, issue, new_comment, post_comment_url,
};
use bot_pass::{now, run, state_dir};
use bot_setup::{BotSetup, bot_setup, bot_setup_with};
use bot_triage::{
    EVIDENCE, KEYS, PAGE, RETRY_AFTER, TRIAGE, answering, bot_comment, failing, fetch_calls,
    github_writes, later, model_calls, run_at, with_page,
};
use catalogue::{LabelsCommandError, load_config};
use catalogue_bot::triage::{
    Claim, SECTION_SUMMARY, evidence_links, section_of_comment, triage_digest,
};
use catalogue_bot::{FormKind, MAX_COMMENT_BYTES, Marker, parse_form};
use catalogue_core::Tier;
use catalogue_github::testing::{Method, Reply, ScriptedTransport};
use serde_json::Value;

/// The submission body.
fn submit() -> String {
    body("submit.md")
}

/// The submission body, with `from` replaced by `to`.
fn submit_body(from: &str, to: &str) -> String {
    submit().replace(from, to)
}

/// A GitHub with issue 12 as `text` and no bot comment yet.
fn first_issue(text: &str) -> ScriptedTransport {
    new_comment(
        github(&Signals::Missing, &[issue(12, text, &["submit"])]),
        12,
        900,
    )
}

/// A GitHub with issue 12 as `text` and bot comment 900 as `posted`.
fn commented_issue(text: &str, posted: &str) -> ScriptedTransport {
    github(
        &Signals::Missing,
        &[issue(12, text, &["submit", "bot-checked"])],
    )
    .route(Method::Get, &comment_url(900), bot_comment(posted))
    .route(Method::Patch, &comment_url(900), Reply::new(200, "{}"))
}

/// The text of the one comment post on issue `number`.
fn posted(transport: &ScriptedTransport, number: u64) -> String {
    let posts: Vec<_> = github_writes(transport)
        .into_iter()
        .filter(|request| request.url == post_comment_url(number))
        .collect();
    assert_eq!(posts.len(), 1, "{posts:?}");
    comment_text(&posts[0])
}

/// The text of the one comment update.
fn patched(transport: &ScriptedTransport) -> String {
    let written = github_writes(transport);
    assert_eq!(written.len(), 1, "{written:?}");
    assert_eq!(written[0].method, Method::Patch);
    comment_text(&written[0])
}

/// The state of issue 12 in the state file.
fn issue_state(setup: &BotSetup) -> Value {
    let text = fs::read_to_string(state_dir(setup).join("bot-state.json")).expect("state file");
    let value: Value = serde_json::from_str(&text).expect("state json");
    value["issues"]["12"].clone()
}

/// The digest of the claim and links of submission `text`, with `web_fetch` on.
fn digest_of(text: &str) -> catalogue_bot::Sha256 {
    digest_with(text, true)
}

/// The digest of the claim and links of submission `text` and the page sources.
fn digest_with(text: &str, web_fetch: bool) -> catalogue_bot::Sha256 {
    let form = parse_form(FormKind::Submit, text).expect("form");
    triage_digest(&Claim::of_form(&form), &evidence_links(&form), web_fetch)
}

/// A first run with the check on that posts comment 900 with a section. Returns its text.
fn checked_first_run(setup: &BotSetup) -> String {
    let transport = answering(with_page(first_issue(&submit()), PAGE), PAGE);
    let report = run_at(setup, &transport, &KEYS, now());
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(
        report.lines,
        ["issue 12: links checked, comment posted, added bot-checked"]
    );
    posted(&transport, 12)
}

/// The comment a run with the check off posts for `text`.
fn unchecked_comment(text: &str) -> String {
    let transport = first_issue(text);
    run(&bot_setup(LABELS), &transport);
    posted(&transport, 12)
}

#[test]
fn a_first_run_posts_the_section_with_the_quote_fenced() {
    let setup = bot_setup_with(LABELS, TRIAGE);
    let transport = answering(with_page(first_issue(&submit()), PAGE), PAGE);
    let report = run_at(&setup, &transport, &KEYS, now());
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        report.lines,
        ["issue 12: links checked, comment posted, added bot-checked"]
    );
    assert_eq!(model_calls(&transport), 1);
    let fetches = fetch_calls(&transport);
    assert_eq!(fetches.len(), 1);
    assert_eq!(fetches[0].json().expect("json")["url"], EVIDENCE);
    let text = posted(&transport, 12);
    let marker = Marker::of_comment(&text).expect("marker");
    assert_eq!(marker.triage, Some(digest_of(&submit())));
    let first_line = text.lines().next().expect("marker line");
    assert!(first_line.contains(" triage=sha256:"), "{first_line}");
    let section = section_of_comment(&text).expect("section");
    assert!(
        section.starts_with(&format!("<details>\n<summary>{SECTION_SUMMARY}</summary>")),
        "{section}"
    );
    assert!(section.contains(&format!(
        "**Link 1:** `{EVIDENCE}`: supports the claim. Quote from the page:\n\n```text\n{PAGE}\n```"
    )));
    let check = text.find("## Catalogue check").expect("report");
    assert!(check < text.find("<details>").expect("section"));
    let saved = issue_state(&setup);
    assert_eq!(saved["triage_sha256"], digest_of(&submit()).to_string());
    assert!(saved.get("triage_retry_after").is_none(), "{saved}");
}

#[test]
fn a_second_identical_run_calls_no_model_and_writes_nothing() {
    let setup = bot_setup_with(LABELS, TRIAGE);
    let text = checked_first_run(&setup);
    let before = issue_state(&setup);
    let transport = commented_issue(&submit(), &text);
    let report = run_at(&setup, &transport, &KEYS, now());
    assert_eq!(report.lines, ["issue 12: up to date"]);
    assert_eq!(model_calls(&transport), 0);
    assert!(fetch_calls(&transport).is_empty());
    assert!(github_writes(&transport).is_empty());
    assert_eq!(issue_state(&setup), before);
}

#[test]
fn an_edit_that_keeps_the_claim_and_links_carries_the_section_forward() {
    let setup = bot_setup_with(LABELS, TRIAGE);
    let text = checked_first_run(&setup);
    let edited = submit_body("https://gizmo.example.com", "https://gizmo.example.org");
    assert_eq!(digest_of(&edited), digest_of(&submit()));
    let transport = commented_issue(&edited, &text);
    let report = run_at(&setup, &transport, &KEYS, now());
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(model_calls(&transport), 0);
    assert!(fetch_calls(&transport).is_empty());
    let updated = patched(&transport);
    assert_eq!(section_of_comment(&updated), section_of_comment(&text));
    let marker = Marker::of_comment(&updated).expect("marker");
    assert_eq!(marker.triage, Some(digest_of(&submit())));
}

#[test]
fn a_changed_evidence_link_is_checked_again() {
    let setup = bot_setup_with(LABELS, TRIAGE);
    let text = checked_first_run(&setup);
    let link = "https://example.com/gizmo-users";
    let edited = submit_body(EVIDENCE, link);
    let transport = answering(with_page(commented_issue(&edited, &text), PAGE), PAGE);
    let report = run_at(&setup, &transport, &KEYS, now());
    assert_eq!(report.lines, ["issue 12: links checked, comment updated"]);
    assert_eq!(model_calls(&transport), 1);
    let fetches = fetch_calls(&transport);
    assert_eq!(fetches.len(), 1);
    assert_eq!(fetches[0].json().expect("json")["url"], link);
    let section = section_of_comment(&patched(&transport)).expect("section");
    assert!(section.contains(&format!("`{link}`")) && !section.contains(EVIDENCE));
}

#[test]
fn a_fetch_key_set_later_checks_the_links_again() {
    let setup = bot_setup_with(LABELS, TRIAGE);
    let transport = answering(first_issue(&submit()), PAGE);
    let report = run_at(&setup, &transport, &KEYS[..2], now());
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        report.warnings,
        [
            "the link check reads GitHub pages only: TEST_WEB_KEY is unset or empty. Set it in the bot environment file"
        ]
    );
    assert_eq!(
        report.lines,
        ["issue 12: links checked, comment posted, added bot-checked"]
    );
    assert_eq!(model_calls(&transport), 0);
    assert!(fetch_calls(&transport).is_empty());
    let text = posted(&transport, 12);
    let marker = Marker::of_comment(&text).expect("marker");
    assert_eq!(marker.triage, Some(digest_with(&submit(), false)));
    assert_ne!(marker.triage, Some(digest_of(&submit())));
    let section = section_of_comment(&text).expect("section");
    assert!(
        section.contains(&format!(
            "**Link 1:** `{EVIDENCE}`: not checked. The page could not be read:"
        )),
        "{section}"
    );

    let transport = answering(with_page(commented_issue(&submit(), &text), PAGE), PAGE);
    let report = run_at(&setup, &transport, &KEYS, now());
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(report.lines, ["issue 12: links checked, comment updated"]);
    assert_eq!(model_calls(&transport), 1);
    assert_eq!(fetch_calls(&transport).len(), 1);
    let updated = patched(&transport);
    let marker = Marker::of_comment(&updated).expect("marker");
    assert_eq!(marker.triage, Some(digest_of(&submit())));
    let section = section_of_comment(&updated).expect("section");
    assert!(
        section.contains(&format!("**Link 1:** `{EVIDENCE}`: supports the claim.")),
        "{section}"
    );
}

#[test]
fn a_section_cut_to_fit_is_not_rewritten_on_the_next_pass() {
    let setup = bot_setup_with(LABELS, TRIAGE);
    let links: Vec<String> = (0..30)
        .map(|index| format!("{EVIDENCE}/{index}/{}", "a".repeat(1500)))
        .collect();
    let text = submit_body(EVIDENCE, &links.join("\n"));
    let transport = answering(with_page(first_issue(&text), PAGE), PAGE);
    let report = run_at(&setup, &transport, &KEYS, now());
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        report.lines,
        ["issue 12: links checked, comment posted, added bot-checked"]
    );
    let comment = posted(&transport, 12);
    assert!(comment.len() <= MAX_COMMENT_BYTES, "{}", comment.len());
    assert!(!comment.contains("**Report cut.**"));
    let section = section_of_comment(&comment).expect("section");
    assert!(section.contains("**Link 1:**"), "{section}");
    assert!(section.contains("**Link check cut.**"), "{section}");
    assert!(!section.contains("**Link 30:**"));
    let before = issue_state(&setup);

    let again = commented_issue(&text, &comment);
    let report = run_at(&setup, &again, &KEYS, now());
    assert_eq!(report.lines, ["issue 12: up to date"]);
    assert_eq!(model_calls(&again), 0);
    assert!(fetch_calls(&again).is_empty());
    assert!(github_writes(&again).is_empty());
    assert_eq!(issue_state(&setup), before);
}

#[test]
fn a_check_no_model_answers_waits_then_runs_again() {
    let setup = bot_setup_with(LABELS, TRIAGE);
    let transport = failing(with_page(first_issue(&submit()), PAGE));
    let report = run_at(&setup, &transport, &KEYS, now());
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(model_calls(&transport), 4, "two attempts at each model");
    let [warning] = report.warnings.as_slice() else {
        panic!("one warning: {:?}", report.warnings);
    };
    assert!(
        warning.starts_with("issue 12: the link check failed: no model answered"),
        "{warning}"
    );
    let text = posted(&transport, 12);
    assert_eq!(text, unchecked_comment(&submit()));
    let saved = issue_state(&setup);
    let retry_after = now().as_second() + RETRY_AFTER;
    assert_eq!(saved["triage_retry_after"], retry_after);
    assert!(saved.get("triage_sha256").is_none(), "{saved}");

    let waiting = commented_issue(&submit(), &text);
    let report = run_at(&setup, &waiting, &KEYS, later(RETRY_AFTER / 2));
    assert_eq!(report.lines, ["issue 12: up to date"]);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(model_calls(&waiting), 0);
    assert!(fetch_calls(&waiting).is_empty());
    assert!(github_writes(&waiting).is_empty());
    assert_eq!(issue_state(&setup)["triage_retry_after"], retry_after);

    let retried = answering(with_page(commented_issue(&submit(), &text), PAGE), PAGE);
    let report = run_at(&setup, &retried, &KEYS, later(RETRY_AFTER + 1));
    assert_eq!(report.lines, ["issue 12: links checked, comment updated"]);
    assert_eq!(model_calls(&retried), 1);
    assert!(section_of_comment(&patched(&retried)).is_some());
    let saved = issue_state(&setup);
    assert!(saved.get("triage_retry_after").is_none(), "{saved}");
    assert_eq!(saved["triage_sha256"], digest_of(&submit()).to_string());
}

#[test]
fn without_a_key_the_check_is_off_and_the_comment_is_unchanged() {
    let expected = unchecked_comment(&submit());
    let setup = bot_setup_with(LABELS, TRIAGE);
    let transport = first_issue(&submit());
    let report = run_at(&setup, &transport, &[], now());
    assert_eq!(model_calls(&transport), 0);
    assert!(fetch_calls(&transport).is_empty());
    assert_eq!(posted(&transport, 12), expected);
    assert_eq!(
        report.warnings,
        [
            "link check provider `ollama` is skipped: TEST_OLLAMA_KEY is unset or empty. Set it in the bot environment file",
            "link check provider `openrouter` is skipped: TEST_OPENROUTER_KEY is unset or empty. Set it in the bot environment file",
            "the link check is off for this run: no provider has an API key. Set a provider key variable, then rerun",
        ]
    );
}

#[test]
fn a_disabled_check_sends_no_model_request_and_posts_todays_comment() {
    let expected = unchecked_comment(&submit());
    let setup = bot_setup_with(
        LABELS,
        &TRIAGE.replace("[triage]\n", "[triage]\nenabled = false\n"),
    );
    let transport = first_issue(&submit());
    let report = run_at(&setup, &transport, &KEYS, now());
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(model_calls(&transport), 0);
    assert!(fetch_calls(&transport).is_empty());
    assert_eq!(posted(&transport, 12), expected);
    assert!(issue_state(&setup).get("triage_sha256").is_none());
}

#[test]
fn a_judgment_checks_its_evidence_links() {
    let setup = bot_setup_with(LABELS, TRIAGE);
    let transport = answering(
        with_page(
            new_comment(
                github(
                    &Signals::Tier("alpha", Tier::Featured),
                    &[issue(13, &body("demotion.md"), &["judgment:demotion"])],
                ),
                13,
                910,
            ),
            "Alpha has had no release since 2024.",
        ),
        "no release since 2024",
    );
    let report = run_at(&setup, &transport, &KEYS, now());
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let text = posted(&transport, 13);
    assert!(text.contains("The entry is `featured` now."), "{text}");
    let section = section_of_comment(&text).expect("section");
    assert!(section.contains(
        "**Link 1:** `https://example.com/decline`: supports the claim. Quote from the page:"
    ));
    assert_eq!(fetch_calls(&transport).len(), 1);
}

#[test]
fn a_bad_triage_config_exits_invalid_and_names_the_field() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    fs::create_dir_all(&base).expect("target dir");
    let dir = tempfile::tempdir_in(base).expect("scratch directory");
    let path = dir.path().join("bot.toml");
    let config = format!(
        "owner = \"o\"\nrepo = \"r\"\napp_id = 1\ninstallation_id = 2\nprivate_key_path = \"k\"\n{}",
        TRIAGE.replace("https://ollama.com/v1", "http://ollama.com/v1")
    );
    fs::write(&path, config).expect("write config");
    let error = load_config(&path).expect_err("refused");
    let message = error.to_string();
    assert!(message.contains("`triage.providers.base_url`"), "{message}");
    assert!(message.contains("Set an https base URL"), "{message}");
    assert_eq!(LabelsCommandError::from(error).exit_code(), 1);
}
