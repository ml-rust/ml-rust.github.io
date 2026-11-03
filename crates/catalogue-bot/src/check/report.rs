//! The check report and its markdown.

/// The heading of every report.
const TITLE: &str = "## Catalogue check";

/// One piece of a report section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// Text the bot writes. It never holds issue text.
    Text(String),
    /// Lines that can hold issue text. They render in a fenced block, so
    /// links, mentions and HTML in them stay plain text.
    Quoted(Vec<String>),
    /// A fenced code block with a language tag.
    Code {
        /// The language tag after the opening fence.
        lang: &'static str,
        /// The code.
        content: String,
    },
}

/// One titled part of a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The section title.
    pub title: &'static str,
    /// The blocks, in order.
    pub blocks: Vec<Block>,
}

/// The result of checking one issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    /// Whether any error blocks the issue.
    pub blocking: bool,
    /// The sections, in a fixed order.
    pub sections: Vec<Section>,
}

impl CheckReport {
    /// The report as GitHub markdown. The same report always gives the same text.
    pub fn markdown(&self) -> String {
        let parts: Vec<String> = self.parts().into_iter().map(|(text, _)| text).collect();
        join(parts)
    }

    /// The report as GitHub markdown in at most `max` bytes.
    ///
    /// A report that fits comes out as [`markdown`](Self::markdown) gives it.
    /// A longer one keeps its parts in order while they fit, cuts the next
    /// fenced block at a line end and closes its fence, then ends with a fixed
    /// notice. A `max` below the notice length gives the notice alone.
    pub fn markdown_within(&self, max: usize) -> String {
        let full = self.markdown();
        if full.len() <= max {
            return full;
        }
        let room = max.saturating_sub(CUT_NOTICE.len() + SEPARATOR.len() + 1);
        let mut kept = Vec::new();
        let mut used = 0_usize;
        for (text, block) in self.parts() {
            let gap = if kept.is_empty() { 0 } else { SEPARATOR.len() };
            let need = used.saturating_add(gap).saturating_add(text.len());
            if need <= room {
                used = need;
                kept.push(text);
                continue;
            }
            let left = room.saturating_sub(used.saturating_add(gap));
            if let Some(cut) = block.and_then(|block| cut_block(block, left)) {
                kept.push(cut);
            }
            break;
        }
        kept.push(CUT_NOTICE.to_owned());
        join(kept)
    }

    /// Every rendered part in order, with the block it renders when it renders one.
    fn parts(&self) -> Vec<(String, Option<&Block>)> {
        let status = if self.blocking {
            "**Result: blocking.** Fix every error below."
        } else {
            "**Result: no blocking error.**"
        };
        let mut parts = vec![(TITLE.to_owned(), None), (status.to_owned(), None)];
        for section in &self.sections {
            parts.push((format!("### {}", section.title), None));
            parts.extend(
                section
                    .blocks
                    .iter()
                    .map(|block| (render_block(block), Some(block))),
            );
        }
        parts
    }
}

/// The gap between two parts.
const SEPARATOR: &str = "\n\n";

/// The last part of a report cut to fit a comment.
const CUT_NOTICE: &str = "**Report cut.** The full report is longer than a GitHub comment allows. \
Shorten the issue body. The bot checks the issue again after the edit.";

/// `parts` joined by blank lines, with a final line break.
fn join(parts: Vec<String>) -> String {
    let mut out = parts.join(SEPARATOR);
    out.push('\n');
    out
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Text(text) => text.clone(),
        Block::Quoted(_) | Block::Code { .. } => fence_input(block)
            .map(|(lang, content)| fenced(lang, &content))
            .unwrap_or_default(),
    }
}

/// The language tag and the content of the fence that `block` renders. `None` for bot text.
fn fence_input(block: &Block) -> Option<(&'static str, String)> {
    match block {
        Block::Text(_) => None,
        Block::Quoted(lines) => Some(("text", lines.join("\n"))),
        Block::Code { lang, content } => Some((*lang, content.trim_end().to_owned())),
    }
}

/// The fenced `block` cut to at most `room` bytes, or `None` when it is bot
/// text or nothing of it fits.
fn cut_block(block: &Block, room: usize) -> Option<String> {
    let (lang, content) = fence_input(block)?;
    let overhead = fenced(lang, &content).len().saturating_sub(content.len());
    let head = prefix_within(&content, room.checked_sub(overhead)?);
    (!head.is_empty()).then(|| fenced(lang, head))
}

/// The longest prefix of `text` within `room` bytes that ends at a line end,
/// or at a character boundary when no line end fits.
fn prefix_within(text: &str, room: usize) -> &str {
    if text.len() <= room {
        return text;
    }
    let head = char_prefix(text, room);
    match head.rfind('\n') {
        Some(at) => head.get(..at).unwrap_or_default(),
        None => head,
    }
}

/// The longest prefix of `text` within `room` bytes that ends at a character boundary.
pub(crate) fn char_prefix(text: &str, room: usize) -> &str {
    if text.len() <= room {
        return text;
    }
    let mut end = room;
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    text.get(..end).unwrap_or_default()
}

/// `lines` sorted, without repeats.
pub(crate) fn sorted(mut lines: Vec<String>) -> Vec<String> {
    lines.sort();
    lines.dedup();
    lines
}

/// `content` in a fenced code block whose fence is longer than any backtick run inside.
pub(crate) fn fenced(lang: &str, content: &str) -> String {
    let longest = content
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}{lang}\n{content}\n{fence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_outgrow_backtick_runs() {
        assert_eq!(fenced("toml", "a = 1"), "```toml\na = 1\n```");
        assert_eq!(fenced("text", "x ```` y"), "`````text\nx ```` y\n`````");
    }

    fn long_report() -> CheckReport {
        CheckReport {
            blocking: true,
            sections: vec![
                Section {
                    title: "Form",
                    blocks: vec![Block::Text("Form: `submit`.".to_owned())],
                },
                Section {
                    title: "Parse problems",
                    blocks: vec![Block::Quoted(vec!["line ``` x".to_owned(); 100])],
                },
                Section {
                    title: "Not checked",
                    blocks: vec![Block::Text("- More.".to_owned())],
                },
            ],
        }
    }

    #[test]
    fn a_report_that_fits_is_not_cut() {
        let report = long_report();
        let full = report.markdown();
        assert_eq!(report.markdown_within(full.len()), full);
    }

    #[test]
    fn a_cut_report_closes_its_fence_and_ends_with_the_notice() {
        let report = long_report();
        let full = report.markdown();
        let max = full.len() / 2;
        let cut = report.markdown_within(max);
        assert!(cut.len() <= max, "{} > {max}", cut.len());
        assert!(cut.ends_with(&format!("{CUT_NOTICE}\n")), "{cut}");
        assert!(cut.contains("### Parse problems\n\n````text\nline ``` x\n"));
        assert!(cut.contains("line ``` x\n````\n\n**Report cut.**"), "{cut}");
        assert!(!cut.contains("### Not checked"));
    }

    #[test]
    fn a_tiny_cap_gives_the_notice_alone() {
        assert_eq!(long_report().markdown_within(3), format!("{CUT_NOTICE}\n"));
    }

    #[test]
    fn prefixes_end_at_a_line_end_or_a_char_boundary() {
        assert_eq!(prefix_within("ab\ncd", 10), "ab\ncd");
        assert_eq!(prefix_within("ab\ncd", 4), "ab");
        assert_eq!(prefix_within("abcd", 3), "abc");
        assert_eq!(prefix_within("aé", 2), "a");
        assert_eq!(char_prefix("aé\nb", 2), "a");
        assert_eq!(char_prefix("ab", 9), "ab");
    }

    #[test]
    fn quoted_lines_stay_inside_a_fence() {
        let report = CheckReport {
            blocking: true,
            sections: vec![Section {
                title: "Parse problems",
                blocks: vec![Block::Quoted(vec![
                    "@someone [x](https://evil.example) ```".to_owned(),
                ])],
            }],
        };
        assert_eq!(
            report.markdown(),
            "## Catalogue check\n\n**Result: blocking.** Fix every error below.\n\n\
             ### Parse problems\n\n\
             ````text\n@someone [x](https://evil.example) ```\n````\n"
        );
    }
}
