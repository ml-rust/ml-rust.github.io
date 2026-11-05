//! The plain text of a README for the search index.

use super::dom::{Step, is_dropped, read_dom, visible};

/// The longest search text, in characters. It keeps the index small.
const MAX_SEARCH_TEXT: usize = 2000;

/// The words of `markdown` for the search index, up to [`MAX_SEARCH_TEXT`] characters.
///
/// Code blocks, image alt text, link targets, dropped elements and bare URLs stay out. The text of
/// other raw HTML elements stays in.
pub(crate) fn readme_search_text(markdown: &str) -> String {
    let dom = read_dom(markdown);
    let mut out = String::new();
    let mut left = MAX_SEARCH_TEXT;
    for step in visible(&dom, |name| name == "pre" || is_dropped(name)) {
        let Step::Text(text) = step else {
            continue;
        };
        for word in text.split_whitespace().filter(|word| !is_url(word)) {
            let chars = word.chars().count();
            if chars + 1 > left {
                return out;
            }
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(word);
            left -= chars + 1;
        }
    }
    out
}

/// Whether `word` reads as a URL.
fn is_url(word: &str) -> bool {
    word.contains("://")
        || word
            .get(..4)
            .is_some_and(|start| start.eq_ignore_ascii_case("www."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_prose_reaches_the_search_text() {
        let markdown = "# Title\n\nA *tensor* `library`. See https://example.org and \
                        [the docs](https://docs.example.org) or www.example.org.\n\n\
                        ```rust\nfn hidden() {}\n```\n\n\
                        ![hidden alt](a.png)\n\n<div>raw html</div>\n\n<!-- hidden -->\n\n\
                        <script>hidden()</script>\n\n<pre>hidden</pre>\n\nEnd.";
        assert_eq!(
            readme_search_text(markdown),
            "Title A tensor library . See and the docs or raw html End."
        );
    }

    #[test]
    fn the_text_of_raw_elements_reaches_the_search_text() {
        assert_eq!(
            readme_search_text(
                "<p align=\"center\"><b>Linear algebra library</b> <i>for Rust.</i></p>"
            ),
            "Linear algebra library for Rust."
        );
        assert_eq!(
            readme_search_text("<div align=\"left\">\n\nBody text."),
            "Body text."
        );
    }

    #[test]
    fn the_text_stops_at_a_word_under_the_limit() {
        let markdown = "word ".repeat(1000);
        let text = readme_search_text(&markdown);
        assert!(text.chars().count() <= MAX_SEARCH_TEXT);
        assert!(text.chars().count() > MAX_SEARCH_TEXT - 10);
        assert!(text.ends_with("word"));
    }

    #[test]
    fn a_readme_with_no_prose_gives_nothing() {
        assert_eq!(readme_search_text("```\ncode\n```"), "");
    }
}
