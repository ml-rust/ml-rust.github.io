//! Turns the parsed document of a README into the tree of allowed elements.

use super::builder::Builder;
use super::dom::{Step, is_dropped, read_dom, visible};
use super::map::open;
use super::resolve::ReadmeBase;
use super::tree::Node;

/// The top-level blocks of `markdown`, with every URL resolved against `base`.
///
/// Dropped elements, refused URLs and the empty blocks they leave are gone. Nothing from the input
/// reaches the tree except text, resolved URLs and validated image sizes.
pub(super) fn parse(markdown: &str, base: &ReadmeBase) -> Vec<Node> {
    let dom = read_dom(markdown);
    let mut builder = Builder::default();
    for step in visible(&dom, is_dropped) {
        match step {
            Step::Open(element) => open(&mut builder, element, base),
            Step::Close => builder.close(),
            Step::Text(text) => builder.text(text),
        }
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::super::builder::MAX_DEPTH;
    use super::super::html::render;
    use super::*;

    fn base() -> ReadmeBase {
        let source = Url::parse("https://github.com/o/r/blob/main/README.md").expect("url");
        ReadmeBase::of(&source, "README.md", "main").expect("base")
    }

    fn element_names(nodes: &[Node], out: &mut Vec<&'static str>) {
        for node in nodes {
            match node {
                Node::Element { element, children } => {
                    out.push(element.name());
                    element_names(children, out);
                }
                Node::Link { children, .. } => element_names(children, out),
                _ => {}
            }
        }
    }

    fn names(markdown: &str) -> Vec<&'static str> {
        let mut out = Vec::new();
        element_names(&parse(markdown, &base()), &mut out);
        out
    }

    /// The HTML the renderer writes for `markdown`.
    fn html(markdown: &str) -> String {
        render(&parse(markdown, &base())).to_string()
    }

    fn depth(nodes: &[Node]) -> usize {
        nodes
            .iter()
            .map(|node| match node {
                Node::Element { children, .. } | Node::Link { children, .. } => 1 + depth(children),
                _ => 1,
            })
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn headings_shift_under_the_about_heading() {
        assert_eq!(
            names("# a\n\n## b\n\n### c\n\n###### d"),
            ["h3", "h4", "h5", "h5"]
        );
    }

    #[test]
    fn a_table_gets_a_head_and_a_body() {
        assert_eq!(
            names("| a |\n| - |\n| b |\n| c |"),
            [
                "table", "thead", "tr", "th", "tbody", "tr", "td", "tr", "td"
            ]
        );
    }

    #[test]
    fn a_code_block_is_code_in_pre_and_keeps_no_language() {
        assert_eq!(names("```rust\nfn a() {}\n```"), ["pre", "code"]);
        assert_eq!(
            html("```rust\nfn a() {}\n```"),
            "<pre><code>fn a() {}\n</code></pre>"
        );
    }

    #[test]
    fn lists_quotes_and_marks_map_to_their_elements() {
        assert_eq!(
            names("- a\n\n1. *b* **c** ~~d~~ `e`\n\n> f"),
            [
                "ul",
                "li",
                "ol",
                "li",
                "em",
                "strong",
                "del",
                "code",
                "blockquote",
                "p"
            ]
        );
    }

    #[test]
    fn the_whitespace_between_blocks_stays_out() {
        assert_eq!(
            html("- a\n  - b\n\n> c"),
            "<ul><li>a<ul><li>b</li></ul></li></ul><blockquote><p>c</p></blockquote>"
        );
    }

    #[test]
    fn a_hard_break_is_one_break() {
        assert_eq!(html("a  \nb"), "<p>a<br>b</p>");
    }

    #[test]
    fn a_refused_link_keeps_its_text_and_a_refused_image_drops_its_alt() {
        let nodes = parse("[kept](javascript:x) ![hidden](data:x) end", &base());
        let [block] = nodes.as_slice() else {
            panic!("{nodes:?}");
        };
        assert_eq!(block.text().trim(), "kept  end");
    }

    #[test]
    fn a_badge_in_a_link_takes_the_link_and_the_paragraph_with_it() {
        let nodes = parse(
            "[![build](https://ci.example.org/b.svg)](https://ci.example.org)\n\nkept",
            &base(),
        );
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].text(), "kept");
    }

    #[test]
    fn a_raw_badge_in_a_link_leaves_nothing() {
        assert_eq!(
            parse(
                "<a href=\"https://x.example\"><img src=\"https://img.shields.io/x.svg\"></a>",
                &base()
            ),
            Vec::new()
        );
    }

    #[test]
    fn the_separators_between_removed_badges_go_with_them() {
        let badge = "<a href=\"https://x.example\"><img src=\"https://img.shields.io/x.svg\"></a>";
        assert_eq!(
            parse(&format!("<p>{badge} | {badge}</p>"), &base()),
            Vec::new()
        );
    }

    #[test]
    fn whole_link_navigation_keeps_its_links_and_separators() {
        assert_eq!(
            html(
                "<p><a href=\"https://a.example\">Web</a> | <a href=\"https://b.example\">Docs</a></p>"
            ),
            "<p><a href=\"https://a.example/\" rel=\"nofollow noopener\">Web</a> | \
             <a href=\"https://b.example/\" rel=\"nofollow noopener\">Docs</a></p>"
        );
    }

    #[test]
    fn an_element_left_open_keeps_every_block_after_it() {
        assert_eq!(
            html("<div align=\"left\">\n\nBody text.\n\n# Next\n\nMore."),
            "<p>Body text.</p><h3>Next</h3><p>More.</p>"
        );
    }

    #[test]
    fn an_unclosed_paragraph_keeps_the_blocks_after_it_and_its_breaks_go() {
        assert_eq!(
            html(
                "<p align=\"center\">\n<br>\n<img src=\"logo.png\"/>\n<br>\n<p>\n\n# Title\n\nBody."
            ),
            "<p><img src=\"https://raw.githubusercontent.com/o/r/main/logo.png\" alt=\"\" \
             loading=\"lazy\"></p><h3>Title</h3><p>Body.</p>"
        );
    }

    #[test]
    fn a_paragraph_with_only_breaks_goes() {
        assert_eq!(parse("<p>\n<br>\n<br>\n</p>", &base()), Vec::new());
    }

    #[test]
    fn raw_marks_map_to_their_elements() {
        assert_eq!(
            html("<p align=\"center\"><b>Linear algebra library</b> <i>for Rust.</i></p>"),
            "<p><strong>Linear algebra library</strong> <em>for Rust.</em></p>"
        );
        assert_eq!(
            html("<p><s>a</s> <strike>b</strike> <tt>c</tt></p>"),
            "<p><del>a</del> <del>b</del> <code>c</code></p>"
        );
    }

    #[test]
    fn a_raw_heading_shifts_like_a_markdown_one() {
        assert_eq!(
            html("<h1 align=\"center\">Whatlang</h1>"),
            "<h3>Whatlang</h3>"
        );
        assert_eq!(html("<h2>a</h2>\n<h6>b</h6>"), "<h4>a</h4><h5>b</h5>");
    }

    #[test]
    fn a_dropped_element_takes_its_text_with_it() {
        for markdown in [
            "<script>alert(1)</script>",
            "<style>x</style>",
            "<svg><text>x</text></svg>",
            "<iframe src=x>y</iframe>",
            "<!-- hidden -->",
            "<template><p>x</p></template>",
            "<noscript>x</noscript>",
            "<form><p>x</p></form>",
            "<video src=\"a.mp4\">x</video>",
            "<title>x</title>",
        ] {
            assert_eq!(parse(markdown, &base()), Vec::new(), "{markdown}");
        }
    }

    #[test]
    fn a_refused_href_keeps_the_text_and_no_attribute_outside_the_allowlist_survives() {
        assert_eq!(
            html("<a href=\"javascript:alert(1)\" onclick=\"x\">kept</a>"),
            "<p>kept</p>"
        );
        assert_eq!(
            html("<a id=\"i\" class=\"c\" style=\"s\" target=\"_top\" href=\"a.md\">t</a>"),
            "<p><a href=\"https://github.com/o/r/blob/main/a.md\" rel=\"nofollow noopener\">t</a></p>"
        );
        assert_eq!(html("<a>kept</a>"), "<p>kept</p>");
    }

    #[test]
    fn an_image_keeps_plain_integer_sizes_only() {
        assert_eq!(
            html("<img src=\"x\" onerror=\"y\" width=\"100%\" height=\"40\">"),
            "<p><img src=\"https://raw.githubusercontent.com/o/r/main/x\" alt=\"\" height=\"40\" \
             loading=\"lazy\"></p>"
        );
        for size in ["0", "-1", "+1", "12345", "1e3", "\" onload=\"x"] {
            let found = html(&format!("<img src=\"x\" width='{size}'>"));
            assert!(!found.contains("width"), "{size}: {found}");
        }
    }

    #[test]
    fn a_picture_keeps_its_image_and_its_source_gives_nothing() {
        assert_eq!(
            html(
                "<picture><source srcset=\"a.png\"><img src=\"b.png\" alt=\"x\" title=\"y\"></picture>"
            ),
            "<p><img src=\"https://raw.githubusercontent.com/o/r/main/b.png\" alt=\"x\" title=\"y\" \
             loading=\"lazy\"></p>"
        );
    }

    #[test]
    fn plain_blocks_keep_their_boundaries() {
        assert_eq!(html("<div>one</div><div>two</div>"), "<p>one</p><p>two</p>");
        assert_eq!(
            html("<div>one<ul><li>two</li></ul>three</div>"),
            "<p>one</p><ul><li>two</li></ul><p>three</p>"
        );
        assert_eq!(html("<div>\n   \n</div>"), "");
    }

    #[test]
    fn a_plain_block_in_an_inline_element_never_lands_in_a_paragraph() {
        assert_eq!(
            html("<div><b><div>one</div></b></div>"),
            "<strong><p>one</p></strong>"
        );
    }

    #[test]
    fn a_pre_keeps_its_whitespace_exactly() {
        assert_eq!(
            html("<pre>\n  a\n   b\n\n</pre>"),
            "<pre>  a\n   b\n\n</pre>"
        );
    }

    #[test]
    fn hostile_nesting_stays_under_the_depth_limit() {
        for markdown in [
            format!("{}a", "> ".repeat(5_000)),
            format!("{}a", "<div>".repeat(5_000)),
            format!("{}a", "<b>".repeat(5_000)),
        ] {
            let nodes = parse(&markdown, &base());
            assert!(depth(&nodes) <= MAX_DEPTH + 1, "{}", depth(&nodes));
            assert_eq!(nodes.iter().map(Node::text).collect::<String>().trim(), "a");
        }
    }

    #[test]
    fn relative_raw_urls_resolve_to_the_forge() {
        assert_eq!(
            html("<img src=\"assets/x.png\">"),
            "<p><img src=\"https://raw.githubusercontent.com/o/r/main/assets/x.png\" alt=\"\" \
             loading=\"lazy\"></p>"
        );
        assert_eq!(
            html("<a href=\"docs/a.md\">t</a>"),
            "<p><a href=\"https://github.com/o/r/blob/main/docs/a.md\" rel=\"nofollow noopener\">t</a></p>"
        );
    }
}
