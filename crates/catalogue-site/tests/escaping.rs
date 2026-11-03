//! Data text reaches the pages escaped.
//!
//! Core refuses non-https links at load, and `SafeHref` turns any other link into `#`.
//! Their unit tests cover links. These tests cover text.

#[path = "common/build.rs"]
mod build;
mod common;
#[path = "common/fixture.rs"]
mod fixture;

use common::{TOPICS, catalogue, pages, thresholds};

/// Markup characters that must never reach a page raw.
const HOSTILE: &str = r#"<script>"'&"#;

/// [`HOSTILE`] as the pages print it.
const ESCAPED: &str = "&#60;script&#62;&#34;&#39;&#38;";

/// The start of the one script tag the site emits: the search page loads its driver.
const SEARCH_SCRIPT_TAG: &str = "<script type=\"module\" src=\"/search/search.";

/// Whether `text` holds a `<script` tag other than the search page's own.
fn holds_raw_script(text: &str) -> bool {
    text.replace(SEARCH_SCRIPT_TAG, "").contains("<script")
}

#[test]
fn the_script_check_allows_only_the_search_script() {
    assert!(!holds_raw_script(
        "<script type=\"module\" src=\"/search/search.0a1b2c3d.js\"></script>"
    ));
    assert!(holds_raw_script("<script>alert(1)</script>"));
    assert!(holds_raw_script(
        "<script type=\"module\" src=\"/x.js\"></script>"
    ));
}

#[test]
fn a_topic_name_with_markup_is_escaped() {
    let topics =
        format!("{TOPICS}\n[[topic]]\nid = \"hostile\"\nname = '''{HOSTILE}alert(1)</script>'''\n");
    let site = pages(&catalogue(&topics, Some(&thresholds(None))));
    for page in ["submit/index.html", "search/index.html"] {
        let html = &site[page];
        assert!(
            html.contains(&format!("{ESCAPED}alert(1)&#60;/script&#62;")),
            "{page}: {html}"
        );
    }
    for (path, text) in &site {
        assert!(!holds_raw_script(text), "{path} holds raw markup");
    }
}

/// The fixture files with markup in a description, a topic summary and a
/// correction finding.
fn hostile_files() -> Vec<(String, String)> {
    let swaps = [
        (
            "\"A tensor library.\"".to_owned(),
            format!("'''{HOSTILE}description</script>'''"),
        ),
        (
            "\"Projects that train or run machine learning models.\"".to_owned(),
            format!("'''{HOSTILE}summary</script>'''"),
        ),
        (
            "\"The README claims GPU support that the code lacks.\"".to_owned(),
            format!("'''{HOSTILE}finding</script>'''"),
        ),
    ];
    let mut files = fixture::files();
    for (from, to) in &swaps {
        let mut hits = 0;
        for (_, contents) in files.iter_mut() {
            if contents.contains(from.as_str()) {
                *contents = contents.replace(from.as_str(), to);
                hits += 1;
            }
        }
        assert_eq!(hits, 1, "the fixture holds {from} once");
    }
    files
}

#[test]
fn hostile_data_text_never_reaches_a_page_raw() {
    for tiered in [true, false] {
        let site = fixture::site(&hostile_files(), tiered);
        let alpha = &site["projects/alpha/index.html"];
        let ml = &site["topics/ml/index.html"];
        let home = &site["index.html"];
        for (page, text, field) in [
            (alpha, "description", "description"),
            (
                &site["topics/tensors/index.html"],
                "description",
                "entry row",
            ),
            (ml, "summary", "topic summary"),
            (home, "summary", "topic card"),
            (alpha, "finding", "correction finding"),
        ] {
            let escaped = format!("{ESCAPED}{text}&#60;/script&#62;");
            assert!(
                page.contains(&escaped),
                "tiered {tiered}: {field} is not escaped"
            );
        }
        for (path, text) in &site {
            assert!(!holds_raw_script(text), "{path} holds raw markup");
            assert!(
                !text.contains("href=\"javascript:"),
                "{path} links a script"
            );
        }
    }
}
