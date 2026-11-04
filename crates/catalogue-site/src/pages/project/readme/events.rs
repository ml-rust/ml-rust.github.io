//! The Markdown events a README page can use: raw HTML is gone, content and all.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use super::raw::RawTags;

/// The events of `markdown` with raw HTML and everything a raw element holds taken out.
///
/// GFM tables and strikethrough are on. Footnotes, math, task lists and heading attributes are off.
pub(super) fn visible_events(markdown: &str) -> Visible<'_> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    Visible {
        parser: Parser::new_ext(markdown, options),
        raw: RawTags::default(),
        skipped: 0,
    }
}

/// The iterator of [`visible_events`].
pub(super) struct Visible<'a> {
    parser: Parser<'a>,
    raw: RawTags,
    /// The tags started inside a raw element that have not ended.
    skipped: usize,
}

impl<'a> Iterator for Visible<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Event<'a>> {
        loop {
            let event = self.parser.next()?;
            match event {
                Event::Start(Tag::HtmlBlock) | Event::End(TagEnd::HtmlBlock) => {}
                Event::Html(text) | Event::InlineHtml(text) => self.raw.read(&text),
                event if self.skipped == 0 && !self.raw.is_open() => return Some(event),
                Event::Start(_) => self.skipped += 1,
                Event::End(_) if self.skipped > 0 => self.skipped -= 1,
                // The container of the raw element ends, and the raw element ends with it.
                event @ Event::End(_) => {
                    self.raw.clear();
                    return Some(event);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text of the events of `markdown`, joined.
    fn text(markdown: &str) -> String {
        visible_events(markdown)
            .filter_map(|event| match event {
                Event::Text(text) | Event::Code(text) => Some(text.into_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn raw_html_drops_with_its_content() {
        for (markdown, kept) in [
            ("a <b>bold</b> c", "a  c"),
            ("<div>\n\n# hidden\n\n- hidden\n\n</div>\n\nkept", "kept"),
            ("<p align=\"center\">\n\n![x](y)\n\n</p>\n\nkept", "kept"),
            ("<script>alert(1)</script>\n\nkept", "kept"),
            ("<!-- hidden -->\n\nkept", "kept"),
            ("x <a href=\"y\">hidden</a> z", "x  z"),
        ] {
            assert_eq!(text(markdown), kept, "{markdown}");
        }
    }

    #[test]
    fn an_unclosed_inline_element_ends_with_its_paragraph() {
        assert_eq!(text("a <b>hidden\n\nkept"), "a kept");
    }

    #[test]
    fn an_unclosed_element_in_a_list_item_ends_with_the_item() {
        assert_eq!(text("- <div>\n\n  hidden\n\n- kept"), "kept");
    }

    #[test]
    fn a_tag_opened_in_a_raw_element_stays_balanced_when_the_element_closes_first() {
        let events: Vec<Event<'_>> = visible_events("<span>*a</span> b* c\n\nkept").collect();
        let starts = events
            .iter()
            .filter(|event| matches!(event, Event::Start(_)))
            .count();
        let ends = events
            .iter()
            .filter(|event| matches!(event, Event::End(_)))
            .count();
        assert_eq!(starts, ends);
    }
}
