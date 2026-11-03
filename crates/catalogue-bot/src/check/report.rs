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
        let status = if self.blocking {
            "**Result: blocking.** Fix every error below."
        } else {
            "**Result: no blocking error.**"
        };
        let mut parts = vec![TITLE.to_owned(), status.to_owned()];
        for section in &self.sections {
            parts.push(format!("### {}", section.title));
            parts.extend(section.blocks.iter().map(render_block));
        }
        let mut out = parts.join("\n\n");
        out.push('\n');
        out
    }
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Text(text) => text.clone(),
        Block::Quoted(lines) => fenced("text", &lines.join("\n")),
        Block::Code { lang, content } => fenced(lang, content.trim_end()),
    }
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
