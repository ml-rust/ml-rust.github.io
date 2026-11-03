//! `.github/labels.toml` holds every label the issue forms and the bot use.

use std::fs;
use std::path::{Path, PathBuf};

use catalogue_bot::{FormKind, LabelsFile, parse_labels};

/// The labels the bot sets on issues.
const BOT_LABELS: [&str; 3] = ["bot-checked", "needs-fix", "approved"];

fn github_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github")
}

fn labels_file() -> LabelsFile {
    let path = github_dir().join("labels.toml");
    let text = fs::read_to_string(&path).expect("labels.toml reads");
    parse_labels(&text).unwrap_or_else(|error| panic!("{}:\n{error}", path.display()))
}

/// The labels of the one-line `labels: ["a", "b"]` entry, or an error for any other shape.
fn line_labels(line: &str) -> Result<Vec<String>, String> {
    let inner = line
        .strip_prefix("labels: [")
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| format!("`{line}` is not `labels: [\"...\"]` on one line"))?;
    inner
        .split(", ")
        .map(|item| {
            item.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .filter(|label| !label.is_empty() && !label.contains('"'))
                .map(str::to_owned)
                .ok_or_else(|| format!("`{item}` in `{line}` is not one quoted label"))
        })
        .collect()
}

/// The labels of each issue form, keyed by template file name.
fn form_labels() -> Vec<(String, Vec<String>)> {
    let dir = github_dir().join("ISSUE_TEMPLATE");
    let mut forms = Vec::new();
    for entry in fs::read_dir(&dir).expect("ISSUE_TEMPLATE reads") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yml") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("utf-8 name")
            .to_owned();
        let text = fs::read_to_string(&path).expect("template reads");
        let mut labels = Vec::new();
        for line in text.lines() {
            if line.trim_start().starts_with("labels:") {
                labels.extend(line_labels(line).unwrap_or_else(|error| panic!("{name}: {error}")));
            }
        }
        forms.push((name, labels));
    }
    forms.sort();
    forms
}

#[test]
fn the_labels_line_shape_is_checked() {
    assert_eq!(
        line_labels(r#"labels: ["submit", "judgment:kind"]"#),
        Ok(vec!["submit".to_owned(), "judgment:kind".to_owned()])
    );
    for bad in [
        "labels:",
        "labels: submit",
        "  labels: [\"submit\"]",
        "labels: [submit]",
        "labels: [\"submit\",\"kind\"]",
        "labels: [\"\"]",
        "labels: ['submit']",
    ] {
        assert!(line_labels(bad).is_err(), "{bad}");
    }
}

#[test]
fn every_form_label_is_in_the_labels_file() {
    let file = labels_file();
    let forms = form_labels();
    assert!(
        forms.iter().any(|(_, labels)| !labels.is_empty()),
        "no form sets a label"
    );
    for (name, labels) in &forms {
        for label in labels {
            let spec = file
                .get(label)
                .unwrap_or_else(|| panic!("{name}: label `{label}` is not in labels.toml"));
            assert_eq!(
                &spec.name, label,
                "{name}: label case differs from labels.toml"
            );
        }
    }
}

#[test]
fn every_form_kind_has_its_template_label_and_a_labels_file_entry() {
    let file = labels_file();
    let forms = form_labels();
    for kind in FormKind::ALL {
        let label = kind.label();
        let template = kind.template();
        let (_, labels) = forms
            .iter()
            .find(|(name, _)| *name == template)
            .unwrap_or_else(|| panic!("no template {template}"));
        assert!(labels.contains(&label), "{template} lacks label `{label}`");
        assert_eq!(
            file.get(&label).map(|spec| spec.name.as_str()),
            Some(label.as_str()),
            "labels.toml lacks `{label}`"
        );
    }
}

#[test]
fn the_bot_labels_are_in_the_labels_file() {
    let file = labels_file();
    for label in BOT_LABELS {
        assert_eq!(
            file.get(label).map(|spec| spec.name.as_str()),
            Some(label),
            "labels.toml lacks `{label}`"
        );
    }
}
