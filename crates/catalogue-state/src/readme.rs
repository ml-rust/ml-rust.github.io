//! The README of a project as plain text, from a release page or its page on the site.
//!
//! A README is written by a project's author. It stays untrusted, and it reaches a model as data.

use crate::facts::clean;

/// The most characters of one README a model reads.
pub const MAX_README: usize = 40_000;

/// The most characters of one changelog a model reads. The newest release comes first in one.
pub const MAX_CHANGELOG: usize = 6000;

/// The part of a project page that opens the README panel.
const PANEL: &str = "class=\"panel readme-panel\"";

/// The part of the panel where the README starts.
const BODY: &str = "<div class=\"readme\">";

/// The plain text of the README in the project page `html`. `None` when the page shows none.
pub fn readme_text(html: &str) -> Option<String> {
    let panel = &html[html.find(PANEL)?..];
    let panel = &panel[..panel.find("</section>")?];
    plain_text(&panel[panel.find(BODY)?..])
}

/// The plain text of a README a crate release publishes as HTML. `None` when it is empty.
pub fn release_readme_text(html: &str) -> Option<String> {
    plain_text(html)
}

/// The plain text of a changelog. `None` when it is empty.
pub fn changelog_text(text: &str) -> Option<String> {
    let cleaned = clean(text, MAX_CHANGELOG);
    (!cleaned.is_empty()).then_some(cleaned)
}

/// `body` with its tags and entities gone, cut to [`MAX_README`]. `None` when nothing is left.
fn plain_text(body: &str) -> Option<String> {
    let mut text = String::with_capacity(body.len());
    let mut in_tag = false;
    for c in body.chars() {
        match c {
            '<' => {
                in_tag = true;
                text.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let decoded = text
        .replace("&#60;", "<")
        .replace("&#62;", ">")
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    let cleaned = clean(&decoded, MAX_README);
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<main><p>Other text</p>
<section class="panel readme-panel" aria-labelledby="about">
<div class="section-head"><h2 id="about">About</h2><p>From the project README</p></div>
<div class="readme"><h1>Tool</h1><p>Writes <code>GGUF</code> &amp; quantizes &#60;fast&#62;.</p></div>
<details><summary>Read the full README</summary><div class="readme"><p>More facts.</p></div></details>
</section><section><p>Later text</p></section></main>"#;

    #[test]
    fn the_text_holds_the_readme_and_nothing_around_it() {
        let text = readme_text(PAGE).expect("readme");
        assert!(text.contains("Tool Writes GGUF & quantizes"), "{text}");
        assert!(text.contains("More facts."), "{text}");
        for outside in [
            "Other text",
            "From the project",
            "Later text",
            "<",
            "class=",
        ] {
            assert!(!text.contains(outside), "{outside}: {text}");
        }
    }

    #[test]
    fn a_page_with_no_readme_gives_none() {
        assert_eq!(readme_text("<main><p>No panel</p></main>"), None);
    }

    #[test]
    fn a_long_readme_is_cut() {
        let long = format!(
            r#"<section class="panel readme-panel"><div class="readme">{}</div></section>"#,
            "word ".repeat(5000)
        );
        let text = readme_text(&long).expect("readme");
        assert!(text.chars().count() <= MAX_README);
    }

    #[test]
    fn the_site_template_still_holds_the_markers_this_reads() {
        let template = include_str!("../../catalogue-site/templates/project_readme.html");
        assert!(template.contains(PANEL));
        assert!(template.contains(BODY));
    }
}
