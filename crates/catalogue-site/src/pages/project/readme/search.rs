//! The plain text of a README for the search index.

use pulldown_cmark::{Event, Tag, TagEnd};

use super::events::visible_events;

/// The longest search text, in characters. It keeps the index small.
const MAX_SEARCH_TEXT: usize = 2000;

/// The words of `markdown` for the search index, up to [`MAX_SEARCH_TEXT`] characters.
///
/// Code blocks, image alt text, link targets, raw HTML and bare URLs stay out.
pub(crate) fn readme_search_text(markdown: &str) -> String {
    let mut out = String::new();
    let mut left = MAX_SEARCH_TEXT;
    let mut hidden: usize = 0;
    for event in visible_events(markdown) {
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::Image { .. }) => hidden += 1,
            Event::End(TagEnd::CodeBlock | TagEnd::Image) => hidden = hidden.saturating_sub(1),
            Event::Text(text) | Event::Code(text) if hidden == 0 => {
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
            _ => {}
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
                        ![hidden alt](a.png)\n\n<div>hidden html</div>\n\n<!-- hidden -->\n\nEnd.";
        assert_eq!(
            readme_search_text(markdown),
            "Title A tensor library . See and the docs or End."
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
