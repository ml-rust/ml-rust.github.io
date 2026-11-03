//! The submit page lists every issue form in `.github/ISSUE_TEMPLATE/` with its own name and description.

use std::fs;
use std::path::PathBuf;

#[path = "common/build.rs"]
mod build;
mod common;

use catalogue_site::ISSUE_FORMS;
use common::{TOPICS, catalogue, pages, thresholds};

/// The `.github/ISSUE_TEMPLATE/` directory of this repository.
fn template_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.github/ISSUE_TEMPLATE")
}

/// The value of the top-level `key: value` line, without surrounding quotes.
fn top_level(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}: ");
    text.lines().find_map(|line| {
        let value = line.strip_prefix(&prefix)?;
        let value = value
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or(value);
        Some(value.to_owned())
    })
}

/// `(file, name, description)` of every issue form, sorted by file name.
fn forms_on_disk() -> Vec<(String, String, String)> {
    let dir = template_dir();
    let mut forms = Vec::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display())) {
        let path = entry.expect("entry").path();
        let file = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("utf-8 name")
            .to_owned();
        if !file.ends_with(".yml") || file == "config.yml" {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{file}: {error}"));
        let name = top_level(&text, "name").unwrap_or_else(|| panic!("{file} has no name line"));
        let description = top_level(&text, "description")
            .unwrap_or_else(|| panic!("{file} has no description line"));
        forms.push((file, name, description));
    }
    forms.sort();
    forms
}

#[test]
fn the_table_matches_the_issue_forms() {
    let mut table: Vec<(String, String, String)> = ISSUE_FORMS
        .iter()
        .map(|form| {
            (
                form.file.to_owned(),
                form.name.to_owned(),
                form.description.to_owned(),
            )
        })
        .collect();
    table.sort();
    assert_eq!(table, forms_on_disk());
}

#[test]
fn the_submit_page_links_every_form() {
    let site = pages(&catalogue(TOPICS, Some(&thresholds(None))));
    let submit = &site["submit/index.html"];
    for form in ISSUE_FORMS {
        let link = format!(
            "<a href=\"https://github.com/ml-rust/ml-rust.github.io/issues/new?template={}\">{}</a>: {}",
            form.file, form.name, form.description
        );
        assert!(submit.contains(&link), "missing {link}");
    }
    assert!(submit.contains("data/maintainers.toml"), "{submit}");
    assert!(submit.contains("<code>approved</code>"), "{submit}");
}

#[test]
fn the_top_level_scan_skips_nested_keys() {
    let text = "name: Form\ndescription: \"Quoted.\"\nbody:\n  - type: input\n    attributes:\n      description: nested\n";
    assert_eq!(top_level(text, "name").as_deref(), Some("Form"));
    assert_eq!(top_level(text, "description").as_deref(), Some("Quoted."));
}
