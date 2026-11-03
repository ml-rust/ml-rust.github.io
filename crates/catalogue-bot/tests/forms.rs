//! The form tables against the issue templates, and the fixture bodies.

mod common;

use std::fs;
use std::path::PathBuf;

use catalogue_bot::{
    Form, FormError, FormKind, JudgmentAnswers, Shape, SlugSource, judgment_draft, parse_form,
    submission,
};
use catalogue_core::Date;
use common::{body, meta};

/// The template text of `kind`.
fn template(kind: FormKind) -> &'static str {
    match kind {
        FormKind::Submit => include_str!("../../../.github/ISSUE_TEMPLATE/submit.yml"),
        FormKind::Correctness => include_str!("../../../.github/ISSUE_TEMPLATE/correctness.yml"),
        FormKind::Supersedes => include_str!("../../../.github/ISSUE_TEMPLATE/supersedes.yml"),
        FormKind::Kind => include_str!("../../../.github/ISSUE_TEMPLATE/kind.yml"),
        FormKind::Category => include_str!("../../../.github/ISSUE_TEMPLATE/category.yml"),
        FormKind::Demotion => include_str!("../../../.github/ISSUE_TEMPLATE/demotion.yml"),
        FormKind::FeaturedApproval => {
            include_str!("../../../.github/ISSUE_TEMPLATE/featured-approval.yml")
        }
        FormKind::Maintainer => include_str!("../../../.github/ISSUE_TEMPLATE/maintainer.yml"),
    }
}

/// One non-markdown body item of a template.
#[derive(Debug, Default)]
struct Item {
    kind: String,
    id: String,
    label: String,
    required: Option<bool>,
    options: Vec<String>,
}

/// A YAML scalar without its surrounding quotes.
fn scalar(text: &str) -> String {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(text)
        .to_owned()
}

/// The `labels` and the body items of a template, read by a line scan.
///
/// The scan knows the layout the templates use: items at `  - type:`,
/// attributes four or six spaces in, options eight spaces in.
fn scan(yaml: &str) -> (Vec<String>, Vec<Item>) {
    let mut labels = Vec::new();
    let mut items: Vec<Item> = Vec::new();
    let mut in_options = false;
    for line in yaml.lines() {
        if let Some(list) = line
            .strip_prefix("labels: [")
            .and_then(|rest| rest.strip_suffix(']'))
        {
            labels = list.split(',').map(scalar).collect();
            continue;
        }
        if let Some(kind) = line.strip_prefix("  - type: ") {
            items.push(Item {
                kind: kind.trim().to_owned(),
                ..Item::default()
            });
            in_options = false;
            continue;
        }
        let Some(item) = items.last_mut() else {
            continue;
        };
        if in_options && let Some(option) = line.strip_prefix("        - ") {
            item.options.push(scalar(option));
            continue;
        }
        in_options = false;
        let trimmed = line.trim();
        if let Some(id) = line.strip_prefix("    id: ") {
            item.id = scalar(id);
        } else if let Some(label) = line.strip_prefix("      label: ") {
            item.label = scalar(label);
        } else if let Some(required) = line.strip_prefix("      required: ") {
            item.required = Some(required.trim() == "true");
        } else if trimmed == "options:" {
            in_options = true;
        }
    }
    items.retain(|item| item.kind != "markdown");
    (labels, items)
}

#[test]
fn tables_match_the_templates() {
    for kind in FormKind::ALL {
        let (labels, items) = scan(template(kind));
        assert_eq!(labels, [kind.label()], "{kind}");
        let fields = kind.fields();
        assert_eq!(items.len(), fields.len(), "{kind}: {items:?}");
        for (item, field) in items.iter().zip(fields) {
            assert_eq!(item.id, field.id, "{kind}");
            assert_eq!(item.label, field.label, "{kind}: {}", field.id);
            assert_eq!(item.required, Some(field.required), "{kind}: {}", field.id);
            let options: Vec<&str> = item.options.iter().map(String::as_str).collect();
            match field.shape {
                Shape::Line => assert_eq!(item.kind, "input", "{kind}: {}", field.id),
                Shape::Text | Shape::List => {
                    assert_eq!(item.kind, "textarea", "{kind}: {}", field.id);
                }
                Shape::Choice(expected) => {
                    assert_eq!(item.kind, "dropdown", "{kind}: {}", field.id);
                    assert_eq!(options, expected, "{kind}: {}", field.id);
                }
            }
            if !matches!(field.shape, Shape::Choice(_)) {
                assert!(options.is_empty(), "{kind}: {}", field.id);
            }
        }
    }
}

#[test]
fn every_template_has_a_form() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.github/ISSUE_TEMPLATE");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("template directory reads")
        .map(|item| {
            item.expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name != "config.yml")
        .collect();
    names.sort();
    let mut expected: Vec<String> = FormKind::ALL.into_iter().map(FormKind::template).collect();
    expected.sort();
    assert_eq!(names, expected);
}

#[test]
fn every_fixture_form_parses_and_converts() {
    let cases = [
        (FormKind::Submit, "submit.md"),
        (FormKind::Correctness, "correctness.md"),
        (FormKind::Supersedes, "supersedes.md"),
        (FormKind::Kind, "kind.md"),
        (FormKind::Category, "category.md"),
        (FormKind::Demotion, "demotion.md"),
        (FormKind::FeaturedApproval, "featured-approval.md"),
        (FormKind::Maintainer, "maintainer.md"),
    ];
    let today = Date::new(2026, 11, 3).expect("date");
    for (kind, name) in cases {
        let form = parse_form(kind, &body(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(form.kind(), kind, "{name}");
        match &form {
            Form::Submit(answers) => {
                submission(answers, &meta(12, "dave"), today).expect("submit converts");
            }
            Form::Judgment(answers) => {
                judgment_draft(answers, &meta(12, "dave")).expect("converts");
            }
        }
    }
}

#[test]
fn a_submit_body_reads_every_field() {
    let Ok(Form::Submit(form)) = parse_form(FormKind::Submit, &body("submit.md")) else {
        panic!("expected a submit form");
    };
    assert_eq!(form.name, "Gizmo");
    assert_eq!(form.crates, ["gizmo-core", "gizmo-nn"]);
    assert_eq!(form.description, "A gizmo that learns\nfrom data.");
    assert_eq!(form.description_source_url, None);
    assert!(form.docker.is_empty());
    let found = submission(
        &form,
        &meta(12, "dave"),
        Date::new(2026, 11, 3).expect("date"),
    )
    .expect("converts");
    assert_eq!(found.slug.as_str(), "gizmo");
    assert_eq!(found.slug_source, SlugSource::RepositoryName);
    assert_eq!(
        found.entry.description.text,
        "A gizmo that learns from data."
    );
}

#[test]
fn crlf_reads_like_lf() {
    for name in ["submit.md", "maintainer.md"] {
        let lf = body(name);
        let crlf = lf.replace('\n', "\r\n");
        let kind = if name == "submit.md" {
            FormKind::Submit
        } else {
            FormKind::Maintainer
        };
        assert_eq!(parse_form(kind, &crlf), parse_form(kind, &lf), "{name}");
    }
}

#[test]
fn bullets_and_brackets_are_stripped() {
    let Ok(Form::Submit(form)) = parse_form(FormKind::Submit, &body("bullets.md")) else {
        panic!("expected a submit form");
    };
    assert_eq!(form.crates, ["gizmo-core", "gizmo-nn"]);
    assert_eq!(form.topics, ["ml"]);
    assert_eq!(form.evidence_of_use, ["https://example.com/uses-gizmo"]);
}

#[test]
fn a_forged_heading_is_rejected() {
    let errors = parse_form(FormKind::Submit, &body("forged-heading.md")).expect_err("rejected");
    assert_eq!(
        errors,
        [FormError::Duplicate {
            label: "Topics",
            count: 2
        }]
    );
}

#[test]
fn a_missing_required_section_is_rejected() {
    let errors = parse_form(FormKind::Submit, &body("missing-required.md")).expect_err("rejected");
    assert_eq!(
        errors,
        [FormError::Missing {
            label: "Repository URL"
        }]
    );
}

#[test]
fn a_dropdown_answer_must_match_exactly() {
    let errors = parse_form(FormKind::Submit, &body("bad-dropdown.md")).expect_err("rejected");
    assert_eq!(
        errors,
        [FormError::BadChoice {
            label: "Kind",
            value: "Library".to_owned(),
            options: &["library", "app"],
        }]
    );
}

#[test]
fn a_body_of_another_form_is_rejected() {
    let errors = parse_form(FormKind::Kind, &body("submit.md")).expect_err("rejected");
    assert!(
        errors.contains(&FormError::Missing {
            label: "Entry slug"
        }),
        "{errors:?}"
    );
}

#[test]
fn a_maintainer_statement_can_omit_evidence() {
    let Ok(Form::Judgment(form)) = parse_form(FormKind::Maintainer, &body("maintainer.md")) else {
        panic!("expected a judgment form");
    };
    assert!(form.evidence.is_empty());
    assert_eq!(
        form.answers,
        JudgmentAnswers::Maintainer {
            status: "finished".to_owned()
        }
    );
}
