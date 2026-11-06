//! The state page shows the report, and a site with no report has no page and no link to it.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;

use catalogue_core::{Date, Movement, StateReport};
use catalogue_site::SiteInputs;
use serde_json::json;

const PAGE: &str = "state/index.html";

fn report(chart: &str) -> StateReport {
    let pick = json!({"project": "alpha", "use_for": "Tensors", "why": "Use [[alpha]] here."});
    let value = json!({
        "schema": 1,
        "written": "2026-11-01",
        "summary": {
            "headline": "Yes, for tensors.",
            "summary": "Use [[alpha]] & keep <Python> for [[gone]].",
            "jobs": [{"topic": "tensors", "readiness": "usable", "python": "partly",
                      "verdict": "It works with [[beta]].", "tools": ["alpha", "gone"]}],
            "use_now": [pick],
            "watch": [pick],
            "build_today": [{"goal": "A solver", "stack": ["alpha"], "why": "It fits.", "watch_out": "It is young."}],
            "holes_filled": [{"hole": "Arrays", "filled_by": ["alpha"], "note": "No Python."}],
            "holes_open": [{"hole": "No evals", "impact": "Teams stop.", "workaround": "Use Python."}],
        },
        "charts": [{"chart": chart, "takeaway": "Most projects wrap native code."}],
    });
    serde_json::from_value(value).expect("report")
}

fn site(state: &StateReport) -> BTreeMap<String, String> {
    let (catalogue, tiers) = fixture::data(&fixture::files());
    build::render(&SiteInputs {
        catalogue: &catalogue,
        report: Some(&tiers),
        changes: &[],
        first_check: None,
        movement: &Movement::NeedsHistory,
        state: Some(state),
    })
}

#[test]
fn a_site_with_no_report_has_no_page_and_no_link() {
    let built = fixture::site(&fixture::files(), true);
    assert!(!built.contains_key(PAGE));
    assert!(!built["index.html"].contains("/state/"));
    assert!(!built["sitemap.xml"].contains("/state/"));
}

#[test]
fn a_report_builds_the_page_the_links_and_the_sitemap_row() {
    let built = site(&report("engine"));
    let page = &built[PAGE];
    for part in [
        "<h1>The state of ML in Rust, as of",
        r#"<p class="state-quote">Yes, for tensors."#,
        "<strong>Build with:</strong>",
        "Written by AI",
        "Usable, with limits",
        "For some steps",
        "Crates to use now",
        "Crates to watch",
        "Gaps Rust has filled",
        "Gaps still open",
        "Most projects wrap native code.",
        "Where the work happens",
        r#"href="/projects/alpha/""#,
        r#"href="/projects/beta/""#,
    ] {
        assert!(page.contains(part), "{part} in {page}");
    }
    assert!(built["index.html"].contains(r#"href="/state/""#));
    assert!(built["rules/index.html"].contains("State of ML"));
    assert!(built["sitemap.xml"].contains("/state/"));
}

#[test]
fn report_text_is_escaped_and_a_lost_project_never_links() {
    let built = site(&report("engine"));
    let page = &built[PAGE];
    assert!(page.contains("&#60;Python&#62;"), "{page}");
    assert!(!page.contains("<Python>"), "{page}");
    assert!(!page.contains("/projects/gone/"), "{page}");
}

#[test]
fn every_chart_of_the_menu_draws() {
    for (chart, title) in [
        ("engine", "Where the work happens"),
        ("releases", "How recently projects shipped"),
        ("gpu", "GPU backends the check can see"),
        ("team", "Who stands behind projects"),
        ("used_by", "Who builds on projects"),
    ] {
        assert!(site(&report(chart))[PAGE].contains(title), "{chart}");
    }
}

#[test]
fn an_old_report_says_so() {
    let mut state = report("engine");
    assert!(!site(&state)[PAGE].contains("more than three weeks old"));
    state.written = Date::new(2020, 1, 1).expect("date");
    assert!(site(&state)[PAGE].contains("more than three weeks old"));
}
