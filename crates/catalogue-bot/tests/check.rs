//! Check reports over in-memory `main` data.

mod common;

use catalogue_bot::{
    CheckInput, CheckReport, Form, FormError, FormKind, check_submission, parse_form,
};
use catalogue_core::{DataSources, Date, SourceFile, Tier};
use common::{body, meta};

const ALPHA: &str = r#"name = "Alpha"
kind = "library"
repo = "https://github.com/ml-rust/alpha"
crates = ["alpha-crate"]
topics = ["ml"]
added = 2026-11-03

[description]
text = "The Alpha project."
source = "human"
"#;

const TOPICS: &str = r#"[[topic]]
id = "ml"
name = "Machine learning"
blurb = "A job."

[[topic]]
id = "tensors"
name = "Tensors"
blurb = "A job."
parent = "ml"
"#;

const CATEGORIES: &str = r#"[[category]]
id = "vector-databases"
name = "Vector databases"
blurb = "Databases that store vectors."
"#;

const NO_CATEGORY: &str = "### Category\n\n_No response_\n";

const MAINTAINERS: &str = r#"[[maintainer]]
login = "carol"
org_member = true
since = 2026-01-01

[[maintainer]]
login = "bob"
org_member = false
since = 2026-02-01
"#;

const JUDGMENT_12: &str = r#"entry = "alpha"
issue = "https://github.com/ml-rust/ml-rust.github.io/issues/12"
reporter = "dave"
approver = "bob"
date = 2026-11-03
evidence = ["https://example.com/proof"]

[judgment]
type = "kind"
value = "demo"
"#;

fn file(path: &str, contents: &str) -> SourceFile {
    SourceFile {
        path: path.to_owned(),
        contents: contents.to_owned(),
    }
}

fn main_data() -> DataSources {
    DataSources {
        entries: vec![file("data/entries/alpha.toml", ALPHA)],
        topics: Some(file("data/topics.toml", TOPICS)),
        categories: Some(file("data/categories.toml", CATEGORIES)),
        maintainers: Some(file("data/maintainers.toml", MAINTAINERS)),
        ..DataSources::default()
    }
}

fn run_with(
    main: &DataSources,
    kind: FormKind,
    text: &str,
    author: &str,
    tier: Option<Tier>,
) -> CheckReport {
    let form = parse_form(kind, text);
    let meta = meta(12, author);
    check_submission(&CheckInput {
        main,
        kind,
        form: &form,
        meta: &meta,
        today: Date::new(2026, 11, 3).expect("date"),
        current_tier: tier,
    })
}

fn run(kind: FormKind, text: &str) -> (bool, String) {
    let report = run_with(&main_data(), kind, text, "dave", None);
    (report.blocking, report.markdown())
}

fn titles(report: &CheckReport) -> Vec<&'static str> {
    report
        .sections
        .iter()
        .map(|section| section.title)
        .collect()
}

#[test]
fn a_valid_submission_passes() {
    let (blocking, text) = run(FormKind::Submit, &body("submit.md"));
    assert!(!blocking, "{text}");
    for needle in [
        "**Result: no blocking error.**",
        "Form: `submit`. Issue: 12. Reporter: `dave`.",
        "### Parse problems\n\nNone.",
        "Path: `data/entries/gizmo.toml`.",
        "```toml\nname = \"Gizmo\"\n",
        "added = 2026-11-03",
        "The new file passes every data rule.",
        "- Slug: `gizmo`. It comes from the repository name.",
        "- Org affiliated: no.",
        "Reporter: dave\nIssue: https://github.com/ml-rust/ml-rust.github.io/issues/12\n",
        "Evidence of use:\n- https://example.com/uses-gizmo",
        "Eligible approvers: `bob`, `carol`.",
        "- Crate existence on crates.io.",
    ] {
        assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
    }
    assert!(!text.contains("LLM triage"), "{text}");
}

#[test]
fn sections_come_in_a_fixed_order() {
    let report = run_with(
        &main_data(),
        FormKind::Submit,
        &body("submit.md"),
        "dave",
        None,
    );
    assert_eq!(
        titles(&report),
        [
            "Form",
            "Parse problems",
            "File",
            "Validation",
            "Submission",
            "Approvers",
            "Not checked"
        ]
    );
    let demotion = run_with(
        &main_data(),
        FormKind::Demotion,
        &body("demotion.md"),
        "dave",
        None,
    );
    assert_eq!(
        titles(&demotion),
        [
            "Form",
            "Parse problems",
            "File",
            "Validation",
            "Demotion",
            "Approvers",
            "Not checked"
        ]
    );
}

#[test]
fn the_same_input_gives_the_same_report() {
    let first = run(FormKind::Submit, &body("submit.md"));
    let second = run(FormKind::Submit, &body("submit.md"));
    assert_eq!(first, second);
}

#[test]
fn the_reporter_and_org_members_cannot_approve() {
    let by_bob = run_with(
        &main_data(),
        FormKind::Submit,
        &body("submit.md"),
        "bob",
        None,
    );
    assert!(by_bob.markdown().contains("Eligible approvers: `carol`."));
    let org = body("submit.md").replace("someone/gizmo", "ml-rust/gizmo");
    let report = run_with(&main_data(), FormKind::Submit, &org, "dave", None);
    let text = report.markdown();
    assert!(text.contains("- Org affiliated: yes."), "{text}");
    assert!(text.contains("Eligible approvers: `bob`."), "{text}");
}

#[test]
fn no_maintainer_means_no_eligible_approver() {
    let mut main = main_data();
    main.maintainers = None;
    let report = run_with(&main, FormKind::Submit, &body("submit.md"), "dave", None);
    assert!(!report.blocking);
    assert!(report.markdown().contains("No eligible approver."));
}

#[test]
fn a_slug_clash_blocks() {
    let mut main = main_data();
    let other = ALPHA
        .replace("ml-rust/alpha", "ml-rust/other")
        .replace("alpha-crate", "other-crate");
    main.entries.push(file("data/entries/gizmo.toml", &other));
    let report = run_with(&main, FormKind::Submit, &body("submit.md"), "dave", None);
    let text = report.markdown();
    assert!(report.blocking, "{text}");
    assert!(text.contains("entry `gizmo` is defined twice"), "{text}");
    assert!(!text.contains("Already broken on main"), "{text}");
}

#[test]
fn a_duplicate_repository_and_crate_block() {
    let mut main = main_data();
    let twin = ALPHA
        .replace("ml-rust/alpha", "someone/gizmo")
        .replace("alpha-crate", "gizmo-core");
    main.entries.push(file("data/entries/twin.toml", &twin));
    let report = run_with(&main, FormKind::Submit, &body("submit.md"), "dave", None);
    let text = report.markdown();
    assert!(report.blocking, "{text}");
    assert!(text.contains("has the same repository"), "{text}");
    assert!(
        text.contains("crate `gizmo-core` is already listed"),
        "{text}"
    );
}

#[test]
fn errors_on_main_are_not_blamed_on_the_submitter() {
    let mut main = main_data();
    main.entries
        .push(file("data/entries/broken.toml", "name = = 1"));
    let report = run_with(&main, FormKind::Submit, &body("submit.md"), "dave", None);
    let text = report.markdown();
    assert!(!report.blocking, "{text}");
    assert!(
        text.contains("The new file passes every data rule."),
        "{text}"
    );
    assert!(text.contains("Already broken on main."), "{text}");
    assert!(text.contains("data/entries/broken.toml: line 1"), "{text}");
    assert!(
        text.contains("Unknown, because main does not load."),
        "{text}"
    );
}

#[test]
fn a_library_with_unknown_topic_and_distribution_blocks() {
    let text = body("submit.md")
        .replace("### Topics\n\nml\n", "### Topics\n\nghost\n")
        .replace(
            "### Homebrew formula\n\n_No response_\n",
            "### Homebrew formula\n\ngizmo\n",
        );
    let (blocking, report) = run(FormKind::Submit, &text);
    assert!(blocking, "{report}");
    assert!(report.contains("topic `ghost` does not exist"), "{report}");
    assert!(
        report.contains("only an app can have `distribution`"),
        "{report}"
    );
}

#[test]
fn a_known_category_goes_in_the_file() {
    let text = body("submit.md").replace(NO_CATEGORY, "### Category\n\nvector-databases\n");
    let (blocking, report) = run(FormKind::Submit, &text);
    assert!(!blocking, "{report}");
    assert!(
        report.contains("category = \"vector-databases\""),
        "{report}"
    );
    assert!(
        report.contains("The new file passes every data rule."),
        "{report}"
    );
}

#[test]
fn no_category_answer_writes_no_field() {
    let (blocking, report) = run(FormKind::Submit, &body("submit.md"));
    assert!(!blocking, "{report}");
    assert!(!report.contains("category ="), "{report}");
}

#[test]
fn an_issue_from_before_the_category_question_still_passes() {
    let text = body("submit.md").replace(NO_CATEGORY, "");
    let (blocking, report) = run(FormKind::Submit, &text);
    assert!(!blocking, "{report}");
}

#[test]
fn an_unknown_category_blocks_and_names_the_next_action() {
    let text = body("submit.md").replace(NO_CATEGORY, "### Category\n\nghost\n");
    let (blocking, report) = run(FormKind::Submit, &text);
    assert!(blocking, "{report}");
    assert!(
        report.contains("category `ghost` does not exist. Use an id from `categories.toml`"),
        "{report}"
    );
}

#[test]
fn a_bad_category_answer_blocks_without_a_file() {
    for answer in ["Not An Id", "vector-databases\ntensor-libraries"] {
        let text = body("submit.md").replace(NO_CATEGORY, &format!("### Category\n\n{answer}\n"));
        let (blocking, report) = run(FormKind::Submit, &text);
        assert!(blocking, "{answer}: {report}");
        assert!(
            report.contains("No file. Fix the parse problems first."),
            "{report}"
        );
    }
}

#[test]
fn a_forged_heading_blocks_and_stays_fenced() {
    let text = body("forged-heading.md").replace("forged-topic", "@someone ```");
    let (blocking, report) = run(FormKind::Submit, &text);
    assert!(blocking, "{report}");
    let problem = FormError::Duplicate {
        label: "Topics",
        count: 2,
    }
    .to_string();
    assert!(
        report.contains(&format!("```text\n{problem}\n```")),
        "{report}"
    );
    assert!(
        report.contains("No file. Fix the parse problems first."),
        "{report}"
    );
    assert!(!report.contains("@someone"), "{report}");
}

#[test]
fn a_label_that_names_another_form_blocks() {
    let form: Result<Form, Vec<FormError>> = parse_form(FormKind::Submit, &body("submit.md"));
    let main = main_data();
    let meta = meta(12, "dave");
    let report = check_submission(&CheckInput {
        main: &main,
        kind: FormKind::Kind,
        form: &form,
        meta: &meta,
        today: Date::new(2026, 11, 3).expect("date"),
        current_tier: None,
    });
    assert!(report.blocking);
    assert!(
        report
            .markdown()
            .contains("the body holds the `submit` form, but the label names `judgment:kind`")
    );
}

#[test]
fn a_judgment_preview_leaves_the_approver_pending() {
    let (blocking, text) = run(FormKind::Correctness, &body("correctness.md"));
    assert!(!blocking, "{text}");
    for needle in [
        "Path: `data/judgments/12.toml`. The approver and the date are pending.",
        "reporter = \"dave\"\n# approver: pending.",
        "type = \"correctness\"",
        "The judgment passes every rule that needs no approver.",
        "Eligible approvers: `bob`, `carol`.",
    ] {
        assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
    }
    assert!(!text.contains("approver = "), "{text}");
}

#[test]
fn judgment_references_are_checked() {
    let (blocking, text) = run(FormKind::Supersedes, &body("supersedes.md"));
    assert!(blocking, "{text}");
    assert!(
        text.contains("replacing entry `beta` does not exist"),
        "{text}"
    );
    let unknown = body("kind.md").replace("alpha", "ghost");
    let (blocking, text) = run(FormKind::Kind, &unknown);
    assert!(blocking, "{text}");
    assert!(text.contains("entry `ghost` does not exist"), "{text}");
}

#[test]
fn a_used_issue_number_blocks() {
    let mut main = main_data();
    main.judgments
        .push(file("data/judgments/12.toml", JUDGMENT_12));
    let report = run_with(&main, FormKind::Kind, &body("kind.md"), "dave", None);
    let text = report.markdown();
    assert!(report.blocking, "{text}");
    assert!(
        text.contains("judgment file `data/judgments/12.toml` already exists"),
        "{text}"
    );
}

#[test]
fn a_demotion_needs_a_featured_entry() {
    let main = main_data();
    let demote = |tier| {
        run_with(
            &main,
            FormKind::Demotion,
            &body("demotion.md"),
            "dave",
            tier,
        )
    };
    let featured = demote(Some(Tier::Featured));
    assert!(!featured.blocking, "{}", featured.markdown());
    assert!(featured.markdown().contains("The demotion can go ahead."));
    let unknown = demote(None);
    assert!(unknown.blocking);
    assert!(unknown.markdown().contains("Current tier unknown."));
    let listed = demote(Some(Tier::Listed));
    assert!(listed.blocking);
    assert!(
        listed
            .markdown()
            .contains("The entry is `listed` now. A demotion needs a `featured` entry.")
    );
}

#[test]
fn a_maintainer_statement_lists_the_ownership_check() {
    let (blocking, text) = run(FormKind::Maintainer, &body("maintainer.md"));
    assert!(!blocking, "{text}");
    assert!(
        text.contains("- Ownership of the crate on crates.io"),
        "{text}"
    );
    assert!(
        text.contains("Eligible approvers: `bob`, `carol`."),
        "{text}"
    );
}
