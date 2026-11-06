//! A README on an entry page: hostile input stays out, relative URLs resolve, the page keeps its shape.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use catalogue_core::{Date, ReadmeFact, Slug};
use catalogue_site::{SiteInputs, search_records};
use url::Url;

const SOURCE: &str = "https://github.com/ml-rust/alpha/blob/main/README.md";

/// The README of `text` at `path`, with its file page at `source`.
fn fact(text: &str, path: &str, source: &str, truncated: bool) -> ReadmeFact {
    ReadmeFact {
        text: text.to_owned(),
        path: path.to_owned(),
        branch: "main".to_owned(),
        truncated,
        fetched: Date::new(2026, 11, 3).expect("date"),
        source: Url::parse(source).expect("source"),
    }
}

/// The page of Alpha with `readme` in its report.
fn page_with(readme: ReadmeFact) -> String {
    let (catalogue, mut report) = fixture::data(&fixture::files());
    let slug = Slug::new("alpha").expect("slug");
    report.entries.get_mut(&slug).expect("alpha").readme = Some(readme);
    let changes = fixture::changes();
    let movement = fixture::movement();
    let site = build::render(&SiteInputs {
        catalogue: &catalogue,
        report: Some(&report),
        changes: &changes,
        first_check: Some(fixture::first_check()),
        movement: &movement,
        state: None,
    });
    site["projects/alpha/index.html"].clone()
}

/// The About section of `page`. Empty when the page leaves it out.
fn about(page: &str) -> &str {
    let Some(start) = page.find("aria-labelledby=\"about\"") else {
        return "";
    };
    let end = page[start..].find("</section>").expect("the end of About");
    &page[start..start + end]
}

/// The README markup of a page built from `markdown` on GitHub at the root, after its wrapper tag.
fn rendered(markdown: &str) -> String {
    let page = page_with(fact(markdown, "README.md", SOURCE, false));
    about(&page)
        .split_once("<div class=\"readme\">")
        .map_or_else(String::new, |(_, body)| body.to_owned())
}

/// The README content of a page built from `markdown`, with a paragraph kept so the section stays.
fn kept(markdown: &str) -> String {
    rendered(&format!("{markdown}\n\nKept."))
}

#[test]
fn hostile_raw_html_leaves_no_tag_attribute_or_hidden_text() {
    for markdown in [
        "<script>alert(1)</script>",
        "<img src=x onerror=alert(1)>",
        "<iframe src=\"https://evil.example\"></iframe>",
        "<svg onload=\"alert(1)\"><text>evil</text></svg>",
        "<a href=\"javascript:alert(1)\" onclick=\"alert(1)\" style=\"evil\">click</a>",
        "<!-- a comment -->",
        "inline <b onclick=\"alert(1)\">bold</b> html",
        "- item\n\n  <div onclick=\"x\">\n\n  <img src=x onerror=y>\n\n  </div>",
    ] {
        let html = kept(markdown);
        assert!(html.contains("<p>Kept.</p>"), "{markdown}: {html}");
        for raw in [
            "<script", "<iframe", "<svg", "onerror", "onclick", "onload", "style=", "alert",
            "evil", "<div", "comment",
        ] {
            assert!(!html.contains(raw), "{markdown} leaves {raw}: {html}");
        }
    }
}

#[test]
fn the_content_of_raw_html_stays_under_the_allowed_tags() {
    for (markdown, want) in [
        (
            "<a href=\"https://example.org\">click</a>",
            "<a href=\"https://example.org/\" rel=\"nofollow noopener\">click</a>",
        ),
        (
            "<div align=\"center\">\n  <h1>Banner</h1>\n</div>",
            "<h3>Banner</h3>",
        ),
        (
            "<p align=\"center\">\n\n![logo](logo.png)\n\n# Banner\n\n</p>",
            "<h3>Banner</h3>",
        ),
        (
            "inline <b onclick=\"x\">bold</b> html",
            "<p>inline <strong>bold</strong> html</p>",
        ),
    ] {
        let html = kept(markdown);
        assert!(html.contains(want), "{markdown}: {html}");
    }
}

#[test]
fn a_leading_centered_banner_vanishes_and_the_prose_stays() {
    let html = rendered(
        "<div align=\"center\">\n\n# Alpha\n\n[![ci](https://ci.example/b.svg)](https://ci.example)\n\n</div>\n\nA tensor library.",
    );
    assert!(html.starts_with("<p>A tensor library.</p></div>"), "{html}");
}

#[test]
fn a_refused_link_scheme_leaves_plain_text() {
    for url in [
        "javascript:alert(1)",
        " JaVaScRiPt:alert(1)",
        "vbscript:x",
        "data:text/html,x",
        "mailto:a@b.c",
        "file:///etc/passwd",
        "#heading",
    ] {
        let html = rendered(&format!("[plain]({url})"));
        assert!(html.contains("<p>plain</p>"), "{url}: {html}");
        assert!(!html.contains("javascript"), "{url}: {html}");
    }
}

#[test]
fn a_protocol_relative_link_reads_as_https_with_the_renderer_rel() {
    let html = rendered("[x](//evil.example)");
    assert!(
        html.contains("<a href=\"https://evil.example/\" rel=\"nofollow noopener\">x</a>"),
        "{html}"
    );
}

#[test]
fn a_refused_image_goes_with_its_alt() {
    for image in [
        "![hidden](data:image/svg+xml,abc)",
        "![hidden](https://img.shields.io/badge/a-b-green)",
        "![hidden](https://docs.rs/alpha/badge.svg)",
        "![hidden](https://crates.io/a.svg)",
        "![hidden](https://ci.example/badge.svg)",
        "![hidden](//evil.example/a.png)",
        "[![hidden](https://ci.example/badge.svg)](https://ci.example)",
    ] {
        let html = kept(image);
        assert!(!html.contains("hidden"), "{image}: {html}");
        assert!(!html.contains("<img"), "{image}: {html}");
        assert!(!html.contains("<a href=\"https://ci"), "{image}: {html}");
        assert!(!html.contains("<p></p>"), "{image}: {html}");
    }
}

#[test]
fn an_attribute_breakout_stays_inside_its_attribute() {
    let html = rendered(r#"![a" onerror="x](y.png "t" onclick="z")"#);
    assert!(!html.contains("\" onerror"), "{html}");
    assert!(!html.contains("\" onclick"), "{html}");
    let html = rendered(r#"![a" onerror="x](y.png)"#);
    assert!(
        html.contains(
            "<img src=\"https://raw.githubusercontent.com/ml-rust/alpha/main/y.png\" alt=\"a&quot; onerror=&quot;x\" loading=\"lazy\">"
        ),
        "{html}"
    );
    let html = rendered("[a](https://example.org \"b\\\" onmouseover=\\\"c\")");
    assert!(!html.contains("\" onmouseover"), "{html}");
    assert!(
        html.contains("title=\"b&quot; onmouseover=&quot;c\""),
        "{html}"
    );
}

#[test]
fn entities_decode_then_escape() {
    let html = rendered("&lt;b&gt; &amp;amp; &quot;x&quot;");
    assert!(html.contains("&lt;b&gt; &amp;amp; &quot;x&quot;"), "{html}");
    assert!(!html.contains("<b>"), "{html}");
}

#[test]
fn relative_links_go_to_the_file_page_from_a_nested_readme() {
    let source = "https://github.com/ml-rust/alpha/blob/main/docs/guide/README.md";
    let root = "https://github.com/ml-rust/alpha/blob/main";
    for (url, want) in [
        ("./a.md", format!("{root}/docs/guide/a.md")),
        ("../a.md", format!("{root}/docs/a.md")),
        ("/a.md", format!("{root}/a.md")),
        ("<a b.md>", format!("{root}/docs/guide/a%20b.md")),
        ("a.md?x=1#y", format!("{root}/docs/guide/a.md?x=1#y")),
    ] {
        let page = page_with(fact(
            &format!("[x]({url})"),
            "docs/guide/README.md",
            source,
            false,
        ));
        let want = format!("<a href=\"{want}\" rel=\"nofollow noopener\">x</a>");
        assert!(about(&page).contains(&want), "{url}: {}", about(&page));
    }
    let page = page_with(fact(
        "[plain](../../../x)",
        "docs/guide/README.md",
        source,
        false,
    ));
    assert!(about(&page).contains("<p>plain</p>"), "{}", about(&page));
}

#[test]
fn a_relative_image_reads_the_raw_file_of_each_forge() {
    for (source, want) in [
        (
            SOURCE,
            "https://raw.githubusercontent.com/ml-rust/alpha/main/a.png",
        ),
        (
            "https://codeberg.org/ml-rust/alpha/src/branch/main/README.md",
            "https://codeberg.org/ml-rust/alpha/raw/branch/main/a.png",
        ),
        (
            "https://gitlab.com/ml-rust/alpha/-/blob/main/README.md",
            "https://gitlab.com/ml-rust/alpha/-/raw/main/a.png",
        ),
    ] {
        let page = page_with(fact("![a](a.png)", "README.md", source, false));
        let want = format!("<img src=\"{want}\" alt=\"a\" loading=\"lazy\">");
        assert!(about(&page).contains(&want), "{source}: {}", about(&page));
    }
}

#[test]
fn a_blob_image_converts_to_the_raw_form() {
    let html = rendered("![a](https://github.com/ml-rust/alpha/blob/main/docs/a.png)");
    assert!(
        html.contains("src=\"https://raw.githubusercontent.com/ml-rust/alpha/main/docs/a.png\""),
        "{html}"
    );
}

#[test]
fn the_section_leads_the_main_column_with_its_source_and_date() {
    let page = page_with(fact("Alpha does tensors.", "README.md", SOURCE, false));
    let start = page.find("<div class=\"project-layout\">").expect("layout");
    let about_at = page.find("id=\"about\"").expect("about");
    let compat_at = page.find("id=\"compatibility\"").expect("compat");
    assert!(start < about_at && about_at < compat_at);
    let section = about(&page);
    assert!(
        section.contains(
            "<a class=\"stat-link\" href=\"https://github.com/ml-rust/alpha/blob/main/README.md\">From the project README</a> · <time datetime=\"2026-11-03\">3 Nov 2026</time>"
        ),
        "{section}"
    );
    assert!(section.contains("<h2 id=\"about\">About</h2>"), "{section}");
    assert!(!section.contains("Read the full README"), "{section}");
}

#[test]
fn a_page_with_no_readme_leaves_the_section_out() {
    let site = fixture::site(&fixture::files(), true);
    assert!(!site["projects/alpha/index.html"].contains("id=\"about\""));
}

#[test]
fn a_readme_with_nothing_left_leaves_the_section_out() {
    let page = page_with(fact(
        "<div align=\"center\"><img src=\"https://ci.example/a.svg\"></div>\n\n[![ci](https://ci.example/b.svg)](https://ci.example)",
        "README.md",
        SOURCE,
        false,
    ));
    assert!(!page.contains("id=\"about\""), "{page}");
}

#[test]
fn a_first_heading_that_repeats_the_name_goes() {
    let html = rendered("# alpha!\n\nBody.\n\n## Install\n\nMore.");
    assert!(!html.contains("alpha!"), "{html}");
    assert!(html.contains("<h4>Install</h4>"), "{html}");
}

#[test]
fn a_long_readme_splits_behind_a_disclosure_and_a_truncated_one_ends_with_a_note() {
    let long = format!("{}\n\nThe rest.", "a".repeat(1300));
    let page = page_with(fact(&long, "README.md", SOURCE, true));
    let section = about(&page);
    let details = section
        .find("<span class=\"disclosure-title\">Read the full README</span>")
        .expect("the disclosure");
    assert!(section[..details].contains("aaaa"), "{section}");
    let rest = &section[details..];
    assert!(rest.contains("<p>The rest.</p>"), "{rest}");
    assert!(
        rest.contains(
            "The README is longer than this. <a href=\"https://github.com/ml-rust/alpha/blob/main/README.md\">Read the rest at the source.</a>"
        ),
        "{rest}"
    );
    let short = page_with(fact("Short.", "README.md", SOURCE, true));
    assert!(about(&short).contains("The README is longer than this."));
    let whole = page_with(fact(&long, "README.md", SOURCE, false));
    assert!(!about(&whole).contains("The README is longer than this."));
}

#[test]
fn a_table_sits_in_a_scroll_wrapper_and_task_markers_stay_text() {
    let html = rendered("| a |\n| - |\n| b |\n\n- [x] done");
    assert!(
        html.contains(
            "<div class=\"table-scroll\"><table><thead><tr><th>a</th></tr></thead><tbody><tr><td>b</td></tr></tbody></table></div>"
        ),
        "{html}"
    );
    assert!(html.contains("[x]"), "{html}");
    assert!(!html.contains("<input"), "{html}");
}

#[test]
fn search_reads_the_prose_of_the_readme_below_the_keywords() {
    let (catalogue, mut report) = fixture::data(&fixture::files());
    let slug = Slug::new("alpha").expect("slug");
    report.entries.get_mut(&slug).expect("alpha").readme = Some(fact(
        "Zebrafish prose. ```not code``` https://example.org\n\n```\nhiddencode\n```",
        "README.md",
        SOURCE,
        false,
    ));
    let records = search_records(&catalogue, Some(&report));
    let alpha = records
        .iter()
        .find(|record| {
            record
                .fields
                .iter()
                .any(|field| field.text.contains("Zebrafish"))
        })
        .expect("the record holds the README");
    let field = alpha
        .fields
        .iter()
        .find(|field| field.text.contains("Zebrafish"))
        .expect("field");
    assert!(field.weight < 1.2, "{}", field.weight);
    assert!(!field.text.contains("hiddencode"), "{}", field.text);
    assert!(!field.text.contains("example.org"), "{}", field.text);
}
