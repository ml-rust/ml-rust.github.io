//! Tracks which raw HTML elements stand open, so their content drops with them.

/// Elements with no closing tag.
const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elements whose closing tag is optional. They never hold the rest of a README back.
const OPTIONAL_CLOSE: [&str; 10] = [
    "colgroup", "dd", "dt", "li", "option", "tbody", "td", "tfoot", "th", "tr",
];

/// Elements whose text is not markup: only their own closing tag ends them.
const RAW_TEXT: [&str; 2] = ["script", "style"];

/// The most raw elements tracked at once. It bounds the search on each closing tag.
const MAX_OPEN: usize = 64;

/// The state of the raw HTML read so far. The parser hands over a block one line at a time.
#[derive(Debug, Default)]
pub(super) struct RawTags {
    /// The open elements, innermost last.
    open: Vec<String>,
    /// The element whose start tag has no `>` yet.
    pending: Option<String>,
    /// Whether a comment has no `-->` yet.
    in_comment: bool,
}

impl RawTags {
    /// Whether a raw element stands open, so the events it holds drop.
    pub(super) fn is_open(&self) -> bool {
        !self.open.is_empty()
    }

    /// Forgets every open element: the container that holds them ended.
    pub(super) fn clear(&mut self) {
        self.open.clear();
        self.pending = None;
        self.in_comment = false;
    }

    /// Reads one piece of raw HTML.
    pub(super) fn read(&mut self, html: &str) {
        let mut rest = html;
        loop {
            if self.in_comment {
                let Some((_, after)) = rest.split_once("-->") else {
                    return;
                };
                self.in_comment = false;
                rest = after;
            }
            if let Some(name) = self.pending.take() {
                let Some(after) = self.finish_tag(name, rest) else {
                    return;
                };
                rest = after;
            }
            if let Some(after) = self.skip_raw_text(rest) {
                rest = after;
            }
            let Some(at) = rest.find('<') else {
                return;
            };
            rest = rest.get(at + 1..).unwrap_or_default();
            if let Some(after) = rest.strip_prefix("!--") {
                self.in_comment = true;
                rest = after;
                continue;
            }
            let closing = rest.starts_with('/');
            let body = rest.strip_prefix('/').unwrap_or(rest);
            let len = body
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == ':'))
                .unwrap_or(body.len());
            let (name, after) = body.split_at(len);
            if !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
                continue;
            }
            let name = name.to_ascii_lowercase();
            if closing {
                self.close(&name);
                rest = after.split_once('>').map_or("", |(_, tail)| tail);
                continue;
            }
            let Some(tail) = self.finish_tag(name, after) else {
                return;
            };
            rest = tail;
        }
    }

    /// Moves past the end of the start tag of `name` and opens it unless it ends with `/>`.
    ///
    /// Returns the text after the tag. `None` when `text` ends first, so the next piece finishes it.
    fn finish_tag<'a>(&mut self, name: String, text: &'a str) -> Option<&'a str> {
        let mut quote = None;
        let mut before = None;
        for (at, c) in text.char_indices() {
            match quote {
                Some(open) if open == c => quote = None,
                Some(_) => {}
                None if c == '"' || c == '\'' => quote = Some(c),
                None if c == '>' => {
                    if before != Some('/') && tracks(&name) && self.open.len() < MAX_OPEN {
                        self.open.push(name);
                    }
                    return Some(text.get(at + 1..).unwrap_or_default());
                }
                None => before = Some(c),
            }
        }
        self.pending = Some(name);
        None
    }

    /// The text from the closing tag on, when the innermost open element is raw text.
    fn skip_raw_text<'a>(&self, text: &'a str) -> Option<&'a str> {
        let top = self.open.last()?;
        if !RAW_TEXT.contains(&top.as_str()) {
            return None;
        }
        let closer = format!("</{top}");
        let at = text
            .as_bytes()
            .windows(closer.len())
            .position(|window| window.eq_ignore_ascii_case(closer.as_bytes()))
            .unwrap_or(text.len());
        text.get(at..)
    }

    /// Closes the innermost open `name` and everything opened inside it.
    fn close(&mut self, name: &str) {
        if let Some(at) = self.open.iter().rposition(|open| open == name) {
            self.open.truncate(at);
        }
    }
}

/// Whether an element of this name holds later content until its closing tag.
fn tracks(name: &str) -> bool {
    !VOID.contains(&name) && !OPTIONAL_CLOSE.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_after(pieces: &[&str]) -> bool {
        let mut raw = RawTags::default();
        for piece in pieces {
            raw.read(piece);
        }
        raw.is_open()
    }

    #[test]
    fn tracking_stops_at_the_limit_and_the_element_stays_open() {
        let mut raw = RawTags::default();
        raw.read(&"<div>".repeat(MAX_OPEN * 2));
        assert_eq!(raw.open.len(), MAX_OPEN);
        assert!(raw.is_open());
    }

    #[test]
    fn a_balanced_or_void_piece_leaves_nothing_open() {
        for pieces in [
            &["<div align=\"center\"><img src=\"x\"></div>"][..],
            &["<br>", "<hr/>", "<img src=\"a>b\">"],
            &["<!-- <div> -->"],
            &["<!-- <div>", "<span> -->"],
            &["<img", " src=\"x\"", ">"],
            &["<script>if (a<b) { c=\"</div>\" }</script>"],
            &["<SCRIPT>x</SCRIPT>"],
            &["</div>"],
            &["<li>a", "<td>b"],
            &["a < b"],
            &["<!DOCTYPE html>"],
            &["<self-closing/>"],
        ] {
            assert!(!open_after(pieces), "{pieces:?}");
        }
    }

    #[test]
    fn an_unclosed_element_stays_open() {
        for pieces in [
            &["<div>"][..],
            &["<div", " align=\"center\">"],
            &["<p align=\"center\">"],
            &["<a href=\"x\">"],
            &["<script>", "a"],
            &["<div><span></span>"],
        ] {
            assert!(open_after(pieces), "{pieces:?}");
        }
    }

    #[test]
    fn a_closing_tag_closes_what_opened_inside_it() {
        assert!(!open_after(&["<div><span>", "</div>"]));
    }

    #[test]
    fn clear_forgets_every_open_element() {
        let mut raw = RawTags::default();
        raw.read("<div><!-- a");
        raw.clear();
        assert!(!raw.is_open());
        raw.read("<b></b>");
        assert!(!raw.is_open());
    }
}
