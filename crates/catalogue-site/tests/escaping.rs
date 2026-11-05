//! Data text reaches the pages escaped.
//!
//! Core refuses non-https links at load, and `SafeHref` turns any other link into `#`.
//! Their unit tests cover links. These tests cover text.

#[path = "common/build.rs"]
mod build;
mod common;
#[path = "common/fixture.rs"]
mod fixture;

use catalogue_core::{ReadmeFact, Slug};
use catalogue_site::SiteInputs;
use common::{TOPICS, catalogue, pages, thresholds};
use url::Url;

/// Markup characters that must never reach a page raw.
const HOSTILE: &str = r#"<script>"'&"#;

/// [`HOSTILE`] as the pages print it.
const ESCAPED: &str = "&#60;script&#62;&#34;&#39;&#38;";

/// The start of every script tag the site emits.
const SCRIPT_TAG_START: &str = "<script type=\"module\" src=\"/";

/// The end of every script tag the site emits.
const SCRIPT_TAG_END: &str = ".js\"></script>";

/// The stem of each script the site emits, before its content hash.
const SCRIPT_STEMS: [&str; 3] = ["search/search.", "assets/list.", "assets/compare."];

/// The length of the content hash in a script path.
const HASH_LEN: usize = 8;

/// The fixed structured data of the home page. It holds no data file text.
const SITE_DATA_TAG: &str = r#"<script type="application/ld+json">{"@context":"https://schema.org","@type":"WebSite","name":"ML Rust catalogue","url":"https://ml-rust.github.io/"}</script>"#;

/// The length of the allowed script tag at the start of `text`, when one starts there.
fn allowed_tag_len(text: &str) -> Option<usize> {
    if text.starts_with(SITE_DATA_TAG) {
        return Some(SITE_DATA_TAG.len());
    }
    let path = text.strip_prefix(SCRIPT_TAG_START)?;
    let stem = SCRIPT_STEMS.iter().find(|stem| path.starts_with(**stem))?;
    let hashed = &path[stem.len()..];
    let hash = hashed.get(..HASH_LEN)?;
    if !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    hashed[HASH_LEN..]
        .starts_with(SCRIPT_TAG_END)
        .then_some(SCRIPT_TAG_START.len() + stem.len() + HASH_LEN + SCRIPT_TAG_END.len())
}

/// Whether `text` holds a `<script` tag other than the exact hashed tags of the site's scripts.
fn holds_raw_script(text: &str) -> bool {
    let mut rest = text;
    while let Some(start) = rest.find("<script") {
        let Some(len) = allowed_tag_len(&rest[start..]) else {
            return true;
        };
        rest = &rest[start + len..];
    }
    false
}

#[test]
fn the_script_check_allows_only_the_hashed_script_tags() {
    for allowed in [
        "<script type=\"module\" src=\"/search/search.0a1b2c3d.js\"></script>",
        "<script type=\"module\" src=\"/assets/list.0a1b2c3d.js\"></script>",
        "<script type=\"module\" src=\"/assets/compare.0a1b2c3d.js\"></script>",
        "a <script type=\"module\" src=\"/assets/list.0a1b2c3d.js\"></script> b \
         <script type=\"module\" src=\"/search/search.0a1b2c3d.js\"></script> c",
        SITE_DATA_TAG,
    ] {
        assert!(!holds_raw_script(allowed), "{allowed}");
    }
    for refused in [
        "<script>alert(1)</script>",
        "<script type=\"application/ld+json\">{\"name\":\"x\"}</script>",
        "<script type=\"module\" src=\"/x.js\"></script>",
        "<script src=\"/assets/list.0a1b2c3d.js\"></script>",
        "<script type=\"module\" src=\"/assets/other.0a1b2c3d.js\"></script>",
        "<script type=\"module\" src=\"/assets/list.js\"></script>",
        "<script type=\"module\" src=\"/assets/list.0a1b2c3z.js\"></script>",
        "<script type=\"module\" src=\"/assets/list.0a1b2c3d4.js\"></script>",
        "<script type=\"module\" src=\"/assets/list.0a1b2c3d.js\" onload=\"x()\"></script>",
        "<script type=\"module\" src=\"/assets/list.0a1b2c3d.js\">alert(1)</script>",
        "<script type=\"module\" src=\"//evil.example/assets/list.0a1b2c3d.js\"></script>",
        "<script type=\"module\" src=\"https://evil.example/assets/list.0a1b2c3d.js\"></script>",
        "<script type=\"module\" src=\"/assets/list.0a1b2c3d.js\"></script><script>alert(1)</script>",
    ] {
        assert!(holds_raw_script(refused), "{refused}");
    }
}

#[test]
fn a_topic_name_with_markup_is_escaped() {
    let topics = format!(
        "{TOPICS}\n[[topic]]\nid = \"hostile\"\nname = '''{HOSTILE}alert(1)</script>'''\nblurb = \"A job.\"\n"
    );
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

/// The fixture files with markup in a description, a topic summary, a category
/// name and blurb, and a correction finding.
fn hostile_files() -> Vec<(String, String)> {
    let swaps = [
        (
            "\"Tensor libraries\"".to_owned(),
            format!("'''{HOSTILE}category</script>'''"),
        ),
        (
            "\"Libraries for multidimensional arrays and their math.\"".to_owned(),
            format!("'''{HOSTILE}blurb</script>'''"),
        ),
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
fn a_hostile_readme_holds_no_raw_script_on_the_built_page() {
    let readme = "<script>alert(1)</script>\n\n\
                  text <script>alert(2)</script> more\n\n\
                  &lt;script&gt;alert(3)&lt;/script&gt;\n\n\
                  `<script>alert(4)</script>`\n\n\
                  ```html\n<script>alert(5)</script>\n```\n\n\
                  [<script>alert(6)</script>](https://example.org \"<script>\")\n\n\
                  ![<script>alert(7)</script>](a.png \"<script>\")\n\n\
                  - <script>alert(8)</script>\n\n\
                  | a |\n| - |\n| <script>alert(9)</script> |\n";
    let (catalogue, mut report) = fixture::data(&fixture::files());
    let slug = Slug::new("alpha").expect("slug");
    let source = Url::parse("https://github.com/ml-rust/alpha/blob/main/README.md").expect("url");
    report.entries.get_mut(&slug).expect("alpha").readme = Some(ReadmeFact {
        text: readme.to_owned(),
        path: "README.md".to_owned(),
        branch: "main".to_owned(),
        truncated: false,
        fetched: fixture::first_check(),
        source,
    });
    let changes = fixture::changes();
    let movement = fixture::movement();
    let site = build::render(&SiteInputs {
        catalogue: &catalogue,
        report: Some(&report),
        changes: &changes,
        first_check: Some(fixture::first_check()),
        movement: &movement,
    });
    let page = &site["projects/alpha/index.html"];
    assert!(page.contains("id=\"about\""), "{page}");
    assert!(page.contains("alert(3)"), "{page}");
    assert!(!holds_raw_script(page), "{page}");
    assert!(!page.contains("alert(1)"), "{page}");
    assert!(!page.contains("alert(2)"), "{page}");
}

#[test]
fn hostile_data_text_never_reaches_a_page_raw() {
    for tiered in [true, false] {
        let site = fixture::site(&hostile_files(), tiered);
        let alpha = &site["projects/alpha/index.html"];
        let ml = &site["topics/ml/index.html"];
        let home = &site["index.html"];
        let category = &site["categories/tensor-libraries/index.html"];
        for (page, text, field) in [
            (alpha, "description", "description"),
            (alpha, "category", "project category link"),
            (home, "category", "home category link"),
            (&site["search/index.html"], "category", "category filter"),
            (category, "category", "category page name"),
            (category, "blurb", "category page blurb"),
            (category, "description", "category page row"),
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
