//! The search page: its form, filters, fallback links and script, and the script's DOM rules.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// The search page file.
const PAGE: &str = "search/index.html";

/// The start of the script tag on the search page.
const SCRIPT_TAG: &str = "<script type=\"module\" src=\"";

/// DOM APIs that parse text as markup. The search script uses none of them.
const MARKUP_APIS: [&str; 4] = [
    "innerHTML",
    "insertAdjacentHTML",
    "outerHTML",
    "document.write",
];

/// The fixture site, with a tier report when `tiered`.
fn site(tiered: bool) -> BTreeMap<String, String> {
    fixture::site(&fixture::files(), tiered)
}

/// The text of the search script in the crate.
fn script_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("search.js");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The checkbox of tag `key` with `value`.
fn checkbox(key: &str, value: &str) -> String {
    format!(
        "<input type=\"checkbox\" id=\"filter-{key}-{value}\" name=\"{key}\" value=\"{value}\">"
    )
}

/// Every API of [`MARKUP_APIS`] that `text` names.
fn markup_apis(text: &str) -> Vec<&'static str> {
    MARKUP_APIS
        .into_iter()
        .filter(|api| text.contains(api))
        .collect()
}

#[test]
fn the_page_holds_the_form_and_the_live_result_list() {
    let site = site(true);
    let page = &site[PAGE];
    for part in [
        "<h1>Search</h1>",
        "<form class=\"search-form\" id=\"search-form\" action=\"/search/\" method=\"get\" role=\"search\">",
        "<input id=\"search-q\" name=\"q\" type=\"search\"",
        "<ol class=\"search-results\" id=\"search-results\" aria-live=\"polite\"></ol>",
        "<legend>Kind</legend>",
        "<legend>Topic</legend>",
        "<legend>Category</legend>",
        "<legend>Tier</legend>",
        "<legend>Organization</legend>",
    ] {
        assert!(page.contains(part), "no {part} in:\n{page}");
    }
}

#[test]
fn the_filters_come_from_the_catalogue_and_match_the_record_tags() {
    let (catalogue, _) = fixture::data(&fixture::files());
    let site = site(true);
    let page = &site[PAGE];
    let mut boxes = vec![
        checkbox("kind", "library"),
        checkbox("kind", "app"),
        checkbox("org", "ml-rust"),
        checkbox("org", "community"),
    ];
    boxes.extend(["featured", "listed", "new", "archived"].map(|tier| checkbox("tier", tier)));
    boxes.extend(
        catalogue
            .topics()
            .iter()
            .map(|topic| checkbox("topic", topic.id.as_str())),
    );
    boxes.extend(
        catalogue
            .categories()
            .iter()
            .map(|category| checkbox("category", category.id.as_str())),
    );
    for found in &boxes {
        assert!(page.contains(found.as_str()), "no {found} in:\n{page}");
    }
    assert!(page.contains("<label for=\"filter-topic-ml\">Machine learning</label>"));
    assert!(page.contains("<span class=\"facet-count\" id=\"filter-topic-ml-count\"></span>"));
    assert!(
        page.contains("<label for=\"filter-category-tensor-libraries\">Tensor libraries</label>")
    );
    assert!(page.contains(
        "<span class=\"facet-count\" id=\"filter-category-tensor-libraries-count\"></span>"
    ));
}

#[test]
fn the_script_keeps_every_filter_group_in_the_url() {
    let source = script_source();
    let keys = source
        .lines()
        .find(|line| line.starts_with("const FILTER_KEYS"))
        .expect("the filter keys");
    let site = site(true);
    for key in [
        "kind", "topic", "category", "license", "runs_on", "tier", "org",
    ] {
        assert!(keys.contains(&format!("\"{key}\"")), "{keys}");
        assert!(site[PAGE].contains(&format!("name=\"{key}\"")), "{key}");
    }
}

#[test]
fn without_a_report_the_page_has_no_tier_filter() {
    let site = site(false);
    let page = &site[PAGE];
    assert!(!page.contains("name=\"tier\""), "{page}");
    assert!(!page.contains("<legend>Tier</legend>"), "{page}");
    assert!(page.contains(&checkbox("kind", "library")));
}

#[test]
fn the_license_and_runs_on_filters_come_from_the_report_and_the_table() {
    let site = site(true);
    let page = &site[PAGE];
    for found in [
        checkbox("license", "apache-2-0-or-mit"),
        checkbox("license", "mit"),
        checkbox("license", "none-stated"),
        checkbox("license", "not-measured"),
        checkbox("runs_on", "cuda"),
        checkbox("runs_on", "metal"),
        checkbox("runs_on", "no-gpu-feature"),
        checkbox("runs_on", "not-measured"),
    ] {
        assert!(page.contains(found.as_str()), "no {found} in:\n{page}");
    }
    assert!(
        page.contains("<label for=\"filter-license-apache-2-0-or-mit\">Apache-2.0 OR MIT</label>"),
        "{page}"
    );
    assert!(page.contains("<label for=\"filter-runs_on-cuda\">CUDA</label>"));
    let untiered = self::site(false);
    assert!(!untiered[PAGE].contains("name=\"license\""));
    assert!(!untiered[PAGE].contains("name=\"runs_on\""));
}

#[test]
fn readers_without_scripts_get_every_topic_link() {
    let (catalogue, _) = fixture::data(&fixture::files());
    let site = site(true);
    let page = &site[PAGE];
    let start = page.find("<noscript>").expect("noscript block");
    let end = page.find("</noscript>").expect("noscript end");
    let fallback = &page[start..end];
    assert!(
        fallback.contains("<a href=\"/topics/\">topics</a>"),
        "{fallback}"
    );
    for topic in catalogue.topics() {
        let link = format!("<a href=\"/topics/{}/\">{}</a>", topic.id, topic.name);
        assert!(fallback.contains(&link), "no {link} in {fallback}");
    }
}

#[test]
fn the_page_loads_the_emitted_hashed_script() {
    let site = site(true);
    let page = &site[PAGE];
    let start = page.find(SCRIPT_TAG).expect("script tag") + SCRIPT_TAG.len();
    let src = &page[start..start + page[start..].find('"').expect("src end")];
    let hash = src
        .strip_prefix("/search/search.")
        .and_then(|rest| rest.strip_suffix(".js"))
        .unwrap_or_else(|| panic!("src {src} is not /search/search.<hash>.js"));
    assert_eq!(hash.len(), 8, "{src}");
    assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()), "{src}");
    let emitted = site
        .get(src.trim_start_matches('/'))
        .unwrap_or_else(|| panic!("the build has no {src}"));
    assert_eq!(*emitted, script_source());
    assert_eq!(page.matches("<script").count(), 1, "{page}");
}

#[test]
fn the_script_never_parses_text_as_markup() {
    let found = markup_apis(&script_source());
    assert!(found.is_empty(), "search.js uses {found:?}");
}

#[test]
fn the_markup_check_finds_each_api() {
    for api in MARKUP_APIS {
        let text = format!("node.{api} = value;");
        assert_eq!(markup_apis(&text), [api]);
    }
    assert!(markup_apis("node.textContent = value;").is_empty());
}
