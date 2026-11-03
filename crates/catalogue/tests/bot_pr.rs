//! `catalogue bot run` turns approved issues into data PRs over a scripted GitHub.

#[path = "common/bot_github.rs"]
mod bot_github;
#[path = "common/bot_pass.rs"]
mod bot_pass;
#[path = "common/bot_pr.rs"]
mod bot_pr;
#[path = "common/bot_setup.rs"]
mod bot_setup;

use bot_github::{
    BOT_LOGIN, LABELS, SHA, Signals, body, comment, comment_text, comment_url, comments_url,
    github, issue_labels_url, new_comment, post_comment_url, url, writes,
};
use bot_pass::run;
use bot_pr::{
    ALPHA_REPO_URL, BASE_TREE, Branch, COMMIT, NEW_TREE, PR, approved, branch_ref_url, calls_from,
    events_url, issue_by, lookups, owners_url, pr_step, public_member_url, pull, pulls_url,
    refs_url,
};
use bot_setup::bot_setup;
use catalogue_bot::Sha256;
use catalogue_core::Tier;
use catalogue_github::testing::{Method, Recorded, Reply, ScriptedTransport};
use serde_json::{Value, json};

/// When `approved` was added.
const APPROVED_AT: &str = "2026-11-03T10:00:00Z";

fn submit() -> String {
    body("submit.md")
}

/// An approved issue by `author` on its own: the listing holds `listed`, the
/// fresh read returns `fresh`, and `approver` added `approved`.
struct Approved<'a> {
    number: u64,
    labels: &'a [&'a str],
    author: &'a str,
    approver: &'a str,
    listed: String,
    fresh: String,
    pulls: Vec<Value>,
    signals: Signals<'a>,
    branch: Branch,
}

impl Approved<'_> {
    /// Submission 12 by `dave`, approved by `bob`, with no PR yet.
    fn submit() -> Self {
        Self {
            number: 12,
            labels: &["submit", "approved"],
            author: "dave",
            approver: "bob",
            listed: submit(),
            fresh: submit(),
            pulls: Vec::new(),
            signals: Signals::Missing,
            branch: Branch::New,
        }
    }

    /// Maintainer statement 20 on `alpha` by `dave`, approved by `bob`.
    fn maintainer() -> Self {
        Self {
            number: 20,
            labels: &["judgment:maintainer", "approved"],
            listed: body("maintainer.md"),
            fresh: body("maintainer.md"),
            ..Self::submit()
        }
    }

    fn github(&self) -> ScriptedTransport {
        let listed = issue_by(self.number, &self.listed, self.labels, self.author);
        let fresh = issue_by(self.number, &self.fresh, self.labels, self.author);
        let events = [approved(1, self.approver, "User", APPROVED_AT)];
        pr_step(
            new_comment(github(&self.signals, &[listed]), self.number, 900),
            self.number,
            &fresh,
            &events,
            &self.pulls,
            &self.branch,
        )
    }
}

fn json_of(request: &Recorded) -> Value {
    request.json().expect("json body")
}

/// The first request with `method` to `target`.
fn find<'a>(written: &'a [Recorded], method: Method, target: &str) -> &'a Recorded {
    written
        .iter()
        .find(|request| request.method == method && request.url == target)
        .unwrap_or_else(|| panic!("no {method:?} {target}: {written:?}"))
}

/// The method and URL of every write.
fn write_calls(transport: &ScriptedTransport) -> Vec<(Method, String)> {
    writes(transport)
        .into_iter()
        .map(|request| (request.method, request.url))
        .collect()
}

/// The posted comment and its label write, with no other write.
fn assert_comment_only(transport: &ScriptedTransport, number: u64) -> String {
    let written = writes(transport);
    let calls: Vec<(Method, &str)> = written
        .iter()
        .map(|request| (request.method, request.url.as_str()))
        .collect();
    assert_eq!(
        calls,
        [
            (Method::Post, post_comment_url(number).as_str()),
            (Method::Post, issue_labels_url(number).as_str()),
        ]
    );
    comment_text(&written[0])
}

#[test]
fn an_approved_submission_opens_a_data_pr() {
    let setup = bot_setup(LABELS);
    let transport = Approved::submit().github();
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        report.lines,
        [
            "issue 12: branch created, opened PR #31, added pr-open, comment posted, added bot-checked"
        ]
    );
    assert_eq!(
        calls_from(&transport, &comments_url(12)),
        [
            (Method::Get, comments_url(12)),
            (Method::Get, events_url(12)),
            (Method::Get, pulls_url(12)),
            (Method::Get, url("/issues/12")),
            (Method::Get, url("/commits/main")),
            (Method::Get, url(&format!("/git/commits/{SHA}"))),
            (Method::Post, url("/git/trees")),
            (Method::Post, url("/git/commits")),
            (Method::Post, refs_url()),
            (Method::Post, url("/pulls")),
            (Method::Post, issue_labels_url(12)),
            (Method::Post, post_comment_url(12)),
            (Method::Post, issue_labels_url(12)),
        ]
    );

    let written = writes(&transport);
    let tree = json_of(find(&written, Method::Post, &url("/git/trees")));
    assert_eq!(tree["base_tree"], BASE_TREE);
    assert_eq!(tree["tree"].as_array().map(Vec::len), Some(1));
    let item = &tree["tree"][0];
    assert_eq!(item["path"], "data/entries/gizmo.toml");
    assert_eq!(item["mode"], "100644");
    assert_eq!(item["type"], "blob");
    let content = item["content"].as_str().expect("content");
    assert!(content.contains("name = \"Gizmo\""), "{content}");
    assert!(content.contains("added = 2026-11-04"), "{content}");

    let commit = json_of(find(&written, Method::Post, &url("/git/commits")));
    assert_eq!(
        commit,
        json!({"message": "data: add gizmo\n\nFrom issue #12.\n", "tree": NEW_TREE, "parents": [SHA]})
    );
    let reference = json_of(find(&written, Method::Post, &refs_url()));
    assert_eq!(
        reference,
        json!({"ref": "refs/heads/bot/issue-12", "sha": COMMIT})
    );

    let pull = json_of(find(&written, Method::Post, &url("/pulls")));
    assert_eq!(pull["title"], "data: add gizmo");
    assert_eq!(pull["head"], "bot/issue-12");
    assert_eq!(pull["base"], "main");
    assert_eq!(pull["maintainer_can_modify"], false);
    let text = pull["body"].as_str().expect("pr body");
    assert!(text.ends_with("Closes #12\n"), "{text}");
    assert!(text.contains("- Reporter: `dave`"), "{text}");
    assert!(text.contains("- Approver: `bob`, on 2026-11-03"), "{text}");
    assert!(
        text.contains(&format!("`{}`", Sha256::of(submit().as_bytes()))),
        "{text}"
    );
    assert!(text.contains("- File: `data/entries/gizmo.toml`"), "{text}");
    assert!(
        text.contains("- <https://example.com/uses-gizmo>"),
        "{text}"
    );
    assert!(!text.contains("A gizmo that learns"), "{text}");

    let labels: Vec<Value> = written
        .iter()
        .filter(|request| request.url == issue_labels_url(12))
        .map(json_of)
        .collect();
    assert_eq!(
        labels,
        [
            json!({"labels": ["pr-open"]}),
            json!({"labels": ["bot-checked"]})
        ]
    );
    let posted = comment_text(find(&written, Method::Post, &post_comment_url(12)));
    assert!(
        posted.contains(&format!("**Data PR:** #{PR} is open.")),
        "{posted}"
    );
}

#[test]
fn a_second_pass_writes_nothing() {
    let setup = bot_setup(LABELS);
    let first = Approved::submit().github();
    run(&setup, &first);
    let posted = comment_text(find(&writes(&first), Method::Post, &post_comment_url(12)));
    let labels = ["submit", "approved", "pr-open", "bot-checked"];
    let listed = issue_by(12, &submit(), &labels, "dave");
    let second = lookups(
        github(&Signals::Missing, &[listed]),
        12,
        &[approved(1, "bob", "User", APPROVED_AT)],
        &[pull(PR, "open", None, None, 12)],
    )
    .route(
        Method::Get,
        &comment_url(900),
        Reply::new(200, comment(900, 12, BOT_LOGIN, "Bot", &posted).to_string()),
    );
    let report = run(&setup, &second);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(report.lines, ["issue 12: PR #31 open"]);
    assert!(writes(&second).is_empty(), "{:?}", writes(&second));
}

#[test]
fn a_merged_pr_is_reported_without_writes() {
    let merged_at = Some("2026-11-03T11:00:00Z");
    let setup = bot_setup(LABELS);
    let mut case = Approved::submit();
    case.pulls = vec![pull(8, "closed", merged_at, merged_at, 12)];
    let transport = case.github();
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let text = assert_comment_only(&transport, 12);
    assert!(text.contains("**Data PR:** #8 is merged."), "{text}");
}

#[test]
fn an_open_pr_is_reported_and_its_missing_label_added() {
    let setup = bot_setup(LABELS);
    let mut case = Approved::submit();
    case.pulls = vec![pull(7, "open", None, None, 12)];
    let transport = case.github();
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let written = writes(&transport);
    let calls: Vec<(Method, &str)> = written
        .iter()
        .map(|request| (request.method, request.url.as_str()))
        .collect();
    assert_eq!(
        calls,
        [
            (Method::Post, issue_labels_url(12).as_str()),
            (Method::Post, post_comment_url(12).as_str()),
            (Method::Post, issue_labels_url(12).as_str()),
        ]
    );
    assert_eq!(written[0].json(), Some(json!({"labels": ["pr-open"]})));
    let text = comment_text(&written[1]);
    assert!(text.contains("**Data PR:** #7 is open."), "{text}");
}

#[test]
fn a_pr_closed_after_the_approval_asks_for_a_new_approval() {
    let setup = bot_setup(LABELS);
    let mut case = Approved::submit();
    case.pulls = vec![pull(6, "closed", None, Some("2026-11-03T12:00:00Z"), 12)];
    let transport = case.github();
    run(&setup, &transport);
    let text = assert_comment_only(&transport, 12);
    assert!(
        text.contains("**Data PR:** #6 closed without a merge."),
        "{text}"
    );
    assert!(
        text.contains("remove the `approved` label and add it again"),
        "{text}"
    );
}

#[test]
fn a_newer_approval_after_a_closed_pr_moves_the_branch() {
    let setup = bot_setup(LABELS);
    let mut case = Approved::submit();
    case.pulls = vec![pull(6, "closed", None, Some("2026-11-03T09:00:00Z"), 12)];
    case.branch = Branch::Exists;
    let transport = case.github();
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(
        report.lines[0].starts_with("issue 12: branch moved, opened PR #31"),
        "{:?}",
        report.lines
    );
    let calls = write_calls(&transport);
    let refs = calls
        .iter()
        .position(|call| *call == (Method::Post, refs_url()))
        .expect("ref create");
    assert_eq!(
        calls.get(refs + 1),
        Some(&(Method::Patch, branch_ref_url(12)))
    );
    assert_eq!(calls.get(refs + 2), Some(&(Method::Post, url("/pulls"))));
    let moved = json_of(find(
        &writes(&transport),
        Method::Patch,
        &branch_ref_url(12),
    ));
    assert_eq!(moved, json!({"sha": COMMIT, "force": true}));
}

/// Runs `case` with the `approved` removal routed and returns the posted comment.
fn refused(case: &Approved<'_>) -> String {
    let setup = bot_setup(LABELS);
    let transport = case.github().route(
        Method::Delete,
        &format!("{}/approved", issue_labels_url(case.number)),
        Reply::new(200, "[]"),
    );
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let calls = write_calls(&transport);
    assert_eq!(
        calls,
        [
            (
                Method::Delete,
                format!("{}/approved", issue_labels_url(case.number))
            ),
            (Method::Post, post_comment_url(case.number)),
            (Method::Post, issue_labels_url(case.number)),
        ]
    );
    let text = comment_text(find(
        &writes(&transport),
        Method::Post,
        &post_comment_url(case.number),
    ));
    assert!(text.contains("**Approval removed.**"), "{text}");
    text
}

#[test]
fn the_approval_rules_remove_a_self_approval_and_an_org_conflict() {
    let mut own = Approved::submit();
    own.author = "bob";
    let text = refused(&own);
    assert!(
        text.contains(
            "approver `bob` cannot approve this submission. Eligible approvers: `carol`."
        ),
        "{text}"
    );
    let mut org = Approved::submit();
    org.listed = submit().replace("someone/gizmo", "ml-rust/gizmo");
    org.fresh = org.listed.clone();
    org.approver = "carol";
    let text = refused(&org);
    assert!(
        text.contains(
            "approver `carol` cannot approve this submission. Eligible approvers: `bob`."
        ),
        "{text}"
    );
}

#[test]
fn a_body_edit_during_the_pass_blocks_the_pr() {
    let setup = bot_setup(LABELS);
    let mut case = Approved::submit();
    case.fresh = submit().replace("A gizmo that learns", "A gizmo that trains");
    let transport = case.github();
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let text = assert_comment_only(&transport, 12);
    assert!(text.contains("**Data PR: blocked.**"), "{text}");
    assert!(
        text.contains("the issue body changed after this pass checked it"),
        "{text}"
    );
}

#[test]
fn an_approved_demotion_opens_a_judgment_pr() {
    let setup = bot_setup(LABELS);
    let case = Approved {
        number: 13,
        labels: &["judgment:demotion", "approved"],
        listed: body("demotion.md"),
        fresh: body("demotion.md"),
        signals: Signals::Tier("alpha", Tier::Featured),
        ..Approved::submit()
    };
    let transport = case.github();
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let written = writes(&transport);
    let tree = json_of(find(&written, Method::Post, &url("/git/trees")));
    assert_eq!(tree["tree"][0]["path"], "data/judgments/13.toml");
    let pull = json_of(find(&written, Method::Post, &url("/pulls")));
    assert_eq!(pull["title"], "data: demotion judgment for alpha");
    let text = pull["body"].as_str().expect("pr body");
    assert!(text.contains("- <https://example.com/decline>"), "{text}");
    assert!(text.ends_with("Closes #13\n"), "{text}");
}

/// Runs maintainer statement 20 with the crates.io and repo answers and,
/// when given, the public membership answer.
fn maintainer(
    crates: Reply,
    repo_owner: Value,
    membership: Option<u16>,
) -> (ScriptedTransport, Vec<String>) {
    let setup = bot_setup(LABELS);
    let mut transport = Approved::maintainer()
        .github()
        .route(Method::Get, &owners_url("alpha-crate"), crates)
        .route(
            Method::Get,
            ALPHA_REPO_URL,
            Reply::new(200, json!({"owner": repo_owner}).to_string()),
        );
    if let Some(status) = membership {
        transport = transport.route(
            Method::Get,
            &public_member_url("ml-rust", "dave"),
            Reply::new(status, ""),
        );
    }
    let report = run(&setup, &transport);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    for request in transport.requests() {
        for forbidden in ["collaborators", "/permission", "/teams/", "/memberships/"] {
            assert!(!request.url.contains(forbidden), "{}", request.url);
        }
    }
    let crates_calls = calls_from(&transport, &owners_url("alpha-crate"));
    assert_eq!(
        crates_calls.first(),
        Some(&(Method::Get, owners_url("alpha-crate")))
    );
    (transport, report.lines)
}

/// The PR body of a maintainer pass that opened a PR.
fn maintainer_pr(transport: &ScriptedTransport) -> String {
    let written = writes(transport);
    let tree = json_of(find(&written, Method::Post, &url("/git/trees")));
    assert_eq!(tree["tree"][0]["path"], "data/judgments/20.toml");
    let content = tree["tree"][0]["content"].as_str().expect("content");
    assert!(content.contains("approver = \"bob\""), "{content}");
    assert!(content.contains("date = 2026-11-03"), "{content}");
    let pull = json_of(find(&written, Method::Post, &url("/pulls")));
    assert_eq!(pull["title"], "data: maintainer judgment for alpha");
    pull["body"].as_str().expect("pr body").to_owned()
}

fn org() -> Value {
    json!({"login": "ml-rust", "type": "Organization"})
}

fn team_owner() -> Reply {
    Reply::new(
        200,
        json!({"users": [{"id": 2, "login": "github:ml-rust:core", "kind": "team"}]}).to_string(),
    )
}

#[test]
fn a_crates_io_owner_confirms_a_maintainer_statement() {
    let crates = Reply::new(
        200,
        json!({"users": [{"id": 1, "login": "Dave", "kind": "user"}]}).to_string(),
    );
    let (transport, _) = maintainer(crates, org(), None);
    let text = maintainer_pr(&transport);
    assert!(
        text.contains("- Ownership: `dave` owns crate `alpha-crate` on crates.io."),
        "{text}"
    );
    assert!(
        !transport
            .requests()
            .iter()
            .any(|request| request.url.contains("public_members"))
    );
}

#[test]
fn the_repo_owner_confirms_a_maintainer_statement() {
    let crates = Reply::new(404, r#"{"errors":[{"detail":"Not Found"}]}"#);
    let (transport, _) = maintainer(crates, json!({"login": "dave", "type": "User"}), None);
    let text = maintainer_pr(&transport);
    assert!(
        text.contains("- Ownership: `dave` owns the repo `ml-rust/alpha` on GitHub."),
        "{text}"
    );
}

#[test]
fn a_public_member_of_the_team_org_confirms_a_maintainer_statement() {
    let (transport, _) = maintainer(team_owner(), org(), Some(204));
    let text = maintainer_pr(&transport);
    assert!(
        text.contains("- Ownership: `dave` is a public member of `ml-rust`, and its team `core` owns crate `alpha-crate` on crates.io."),
        "{text}"
    );
}

#[test]
fn an_unconfirmed_owner_blocks_the_maintainer_pr() {
    let (transport, lines) = maintainer(team_owner(), org(), Some(404));
    assert_eq!(
        lines,
        ["issue 20: PR blocked, comment posted, added bot-checked"]
    );
    let text = assert_comment_only(&transport, 20);
    assert!(text.contains("**Data PR: blocked.**"), "{text}");
    for needle in [
        "crates.io: `dave` is not a user owner of `alpha-crate`.",
        "GitHub: `dave` does not own the repo `ml-rust/alpha`.",
        "Org membership: `dave` is not a public member of `ml-rust`.",
        "remove the `approved` label and add it again",
    ] {
        assert!(text.contains(needle), "{needle}: {text}");
    }
}
