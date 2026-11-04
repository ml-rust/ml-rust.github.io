//! The changes page, list rows and entry pages show movement, and say when it cannot be measured.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;

use catalogue_core::Movement;

const NEEDS_HISTORY: &str = "Movement needs more history.";

fn tiered() -> BTreeMap<String, String> {
    fixture::site(&fixture::files(), true)
}

/// Asserts that `page` holds each of `parts`, in order.
fn in_order(page: &str, parts: &[&str]) {
    let mut rest = page;
    for part in parts {
        let Some(at) = rest.find(part) else {
            panic!("missing {part:?} after the earlier parts in:\n{page}");
        };
        rest = &rest[at + part.len()..];
    }
}

#[test]
fn the_changes_page_shows_the_tier_changes_above_movement() {
    let site = tiered();
    in_order(
        &site["changes/index.html"],
        &[
            "<h2 id=\"tier-changes\">Tier changes</h2>",
            "<ol class=\"timeline\">",
            "<h2 id=\"movement\">Movement</h2>",
            "the check on <time datetime=\"2026-11-04\">4 Nov 2026</time> with the check on <time datetime=\"2026-10-05\">5 Oct 2026</time>",
            "It never affects a tier.",
            "<h3 id=\"new-this-month\">New this month</h3>",
            "<a href=\"/projects/delta/\">Delta</a></td><td><time datetime=\"2026-11-02\">",
            "<h3 id=\"rising\">Rising</h3>",
            "<a href=\"/projects/alpha/\">Alpha</a></td><td>5</td><td>9.36</td>",
            "<h3 id=\"falling\">Falling</h3>",
            "<a href=\"/projects/beta/\">Beta</a></td><td>40</td><td>20.5</td>",
        ],
    );
}

#[test]
fn an_entry_the_catalogue_no_longer_holds_has_no_movement_row() {
    let site = tiered();
    let changes = &site["changes/index.html"];
    assert!(!changes.contains("<td>90</td>"), "{changes}");
    assert!(!changes.contains("/projects/omega/"), "{changes}");
}

#[test]
fn rows_and_entry_pages_show_a_plain_word() {
    let site = tiered();
    for (page, word) in [
        ("topics/tensors/index.html", "Rising"),
        ("topics/tensors/index.html", "Recently added"),
        ("topics/ml/index.html", "Falling"),
        ("categories/tensor-libraries/index.html", "Rising"),
        ("categories/inference-runtimes/index.html", "Falling"),
        ("projects/alpha/index.html", "Rising"),
        ("projects/beta/index.html", "Falling"),
        ("projects/delta/index.html", "Recently added"),
    ] {
        let span = format!("<span class=\"entry-movement\">{word}</span>");
        assert!(site[page].contains(&span), "{page} lacks {word}");
    }
    // Gamma's own header holds no word. Its also-in rows can show another entry's word.
    let gamma = &site["projects/gamma/index.html"];
    let header = &gamma[..gamma.find("<h2").expect("a section")];
    assert!(!header.contains("entry-movement"), "{header}");
}

#[test]
fn too_little_history_gives_a_plain_line_and_no_word() {
    let site = fixture::site_with(&fixture::files(), true, &Movement::NeedsHistory);
    for page in [
        "changes/index.html",
        "topics/tensors/index.html",
        "categories/tensor-libraries/index.html",
        "projects/alpha/index.html",
    ] {
        assert!(site[page].contains(NEEDS_HISTORY), "{page}");
        assert!(!site[page].contains("entry-movement"), "{page}");
    }
    let changes = &site["changes/index.html"];
    assert!(!changes.contains("New this month"), "{changes}");
}

#[test]
fn without_a_report_no_page_shows_movement() {
    let site = fixture::site(&fixture::files(), false);
    for (path, text) in site.iter().filter(|(path, _)| path.ends_with(".html")) {
        assert!(!text.contains("entry-movement"), "{path}");
        assert!(!text.contains("Movement needs more history"), "{path}");
    }
    assert!(!site["changes/index.html"].contains("id=\"movement\""));
}

#[test]
fn the_rules_page_shows_the_movement_values() {
    let site = tiered();
    let rules = &site["rules/index.html"];
    assert!(
        rules.contains("<code>movement.window_days</code>"),
        "{rules}"
    );
    assert!(rules.contains("It never affects a tier."), "{rules}");
}
