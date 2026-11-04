//! Entry pages and rows show the facts a person reads to choose a project.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;

const ALPHA: &str = "projects/alpha/index.html";
const BETA: &str = "projects/beta/index.html";
const TOPIC: &str = "topics/tensors/index.html";
const CATEGORY: &str = "categories/tensor-libraries/index.html";

fn tiered() -> BTreeMap<String, String> {
    fixture::site(&fixture::files(), true)
}

fn untiered() -> BTreeMap<String, String> {
    fixture::site(&fixture::files(), false)
}

/// Asserts that `page` holds each of `parts`, in order.
fn in_order(page: &str, parts: &[&str]) {
    let mut rest = page;
    for part in parts {
        let Some(at) = rest.find(part) else {
            panic!("no {part} after the parts before it in:\n{page}");
        };
        rest = &rest[at + part.len()..];
    }
}

#[test]
fn the_glance_list_comes_before_the_description_in_the_stated_order() {
    let site = tiered();
    in_order(
        &site[ALPHA],
        &[
            "<h2 id=\"at-a-glance\">At a glance</h2>",
            "Tier: <span class=\"tier-badge",
            "<li>License: Apache-2.0 OR MIT</li>",
            "<li>Runs on: CUDA, Metal</li>",
            "<li>Native code: Binds native code: openssl-sys. Has Python bindings: alpha-extra needs Python</li>",
            "<li>Active users: 9</li>",
            "<li>Dependents: 12</li>",
            "<li>Contributors: 38</li>",
            "<li>Install: <code>cargo add alpha-crate</code> <a href=\"https://crates.io/crates/alpha-crate\">crates.io</a></li>",
            "<h2 id=\"description\">",
        ],
    );
}

#[test]
fn an_unknown_value_reads_not_measured_and_an_app_installs_from_its_channel() {
    let site = tiered();
    let page = &site[BETA];
    assert!(page.contains("<li>License: None stated</li>"), "{page}");
    assert!(page.contains("<li>Runs on: Not measured</li>"), "{page}");
    assert!(
        page.contains("<li>Native code: Not measured</li>"),
        "{page}"
    );
    assert!(
        page.contains("<li>Active users: Not measured</li>"),
        "{page}"
    );
    assert!(
        page.contains("<li>Contributors: Not measured</li>"),
        "{page}"
    );
    assert!(page.contains("<code>brew install beta</code>"), "{page}");
}

#[test]
fn the_signal_table_keeps_the_precise_value_and_each_fact_shows_its_source() {
    let site = tiered();
    let page = &site[ALPHA];
    assert!(page.contains("<td>9.36</td>"), "{page}");
    in_order(
        page,
        &[
            "<th scope=\"row\">Contributors</th><td>38</td>",
            "https://github.com/ml-rust/alpha/graphs/contributors",
            "<td>No</td>",
        ],
    );
}

#[test]
fn rows_on_topic_and_category_pages_show_the_facts_as_whole_numbers() {
    let site = tiered();
    // The home page's Featured rows compare the same facts.
    for page in [TOPIC, CATEGORY, "index.html"] {
        in_order(
            &site[page],
            &[
                "<tr class=\"entry-row\" data-slug=\"alpha\"",
                "data-users=\"9\"",
                "data-dependents=\"12\"",
                "<td class=\"cell-users\" data-label=\"Active users\"><span class=\"fact\">9</span></td>",
                "<td class=\"cell-gpu\" data-label=\"Runs on (GPU)\"><span class=\"fact\">CUDA, Metal</span></td>",
                "<td class=\"cell-license\" data-label=\"License\"><span class=\"fact\">Apache-2.0 OR MIT</span></td>",
            ],
        );
    }
}

#[test]
fn a_row_says_whether_native_code_is_required_and_keeps_the_exact_fact() {
    let site = tiered();
    let native = "Binds native code: openssl-sys";
    let cell = format!(
        "<td class=\"cell-native\" data-label=\"Native code\"><span class=\"fact\" title=\"{native}\"><span aria-hidden=\"true\">Required</span><span class=\"visually-hidden\">{native}</span></span></td>"
    );
    assert!(site[TOPIC].contains(&cell), "{}", site[TOPIC]);
}

#[test]
fn python_bindings_show_as_a_tag_beside_the_name_with_the_exact_fact() {
    let site = tiered();
    let page = &site[TOPIC];
    in_order(
        page,
        &[
            "/projects/alpha/\">Alpha</a>",
            "<span class=\"row-tag\" title=\"Has Python bindings: alpha-extra needs Python\">Python bindings</span>",
            "<p class=\"row-description\">",
        ],
    );
    let cell = &page[page.find("class=\"cell-native\"").expect("a native cell")..];
    let cell = &cell[..cell.find("</td>").expect("the end of the cell")];
    assert!(!cell.contains("Python"), "{cell}");
}

#[test]
fn a_build_without_a_report_reads_not_measured_and_shows_no_tier() {
    let site = untiered();
    let page = &site[ALPHA];
    assert!(page.contains("<li>License: Not measured</li>"), "{page}");
    assert!(!page.contains("<li>Tier:"), "{page}");
    assert!(
        page.contains("<code>cargo add alpha-crate</code>"),
        "{page}"
    );
    in_order(
        &site[TOPIC],
        &[
            "<td class=\"cell-gpu\" data-label=\"Runs on (GPU)\"><span class=\"fact fact-quiet\">Not measured</span></td>",
            "<td class=\"cell-license\" data-label=\"License\"><span class=\"fact fact-quiet\">Not measured</span></td>",
        ],
    );
}

#[test]
fn the_section_names_the_shared_topic_and_nothing_more() {
    let site = tiered();
    in_order(
        &site[ALPHA],
        &[
            "<h2 id=\"also-in\">Also in these topics</h2>",
            "<a href=\"/projects/delta/\">Delta</a>",
            "Also in <a href=\"/topics/tensors/\">Tensors</a>.",
            "<a href=\"/projects/gamma/\">Gamma</a>",
            "Also in <a href=\"/topics/tensors/\">Tensors</a>.",
        ],
    );
    let page = &site[BETA];
    assert!(
        !page.contains("id=\"also-in\""),
        "no other entry is in its topic: {page}"
    );
}

#[test]
fn the_rules_page_says_these_facts_never_count_and_how_each_is_measured() {
    let site = tiered();
    let page = &site["rules/index.html"];
    assert!(page.contains("They never count toward a tier."), "{page}");
    for label in [
        "License",
        "Runs on",
        "Native code",
        "Python bindings",
        "Dependents",
        "Contributors",
    ] {
        assert!(
            page.contains(&format!("<strong>{label}</strong>")),
            "{label}"
        );
    }
}
