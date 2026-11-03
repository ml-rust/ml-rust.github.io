//! The text of a bot check comment.

use super::marker::Marker;
use crate::check::CheckReport;

/// The most bytes a bot comment holds. GitHub refuses a comment past 65,536 characters.
pub const MAX_COMMENT_BYTES: usize = 65_000;

/// The note a comment carries when the bot removed `approved`.
const APPROVAL_DROPPED: &str = "**Approval removed.** The issue body changed after the last check, \
so the `approved` label no longer covers it. A maintainer must read the new text and approve again.";

/// The comment for `report`: the marker line, the approval note when
/// `approval_dropped`, then the report.
///
/// The text stays within [`MAX_COMMENT_BYTES`]. Only the report is cut, so the
/// marker line is always whole. Issue text appears only inside the fenced
/// blocks of the report.
pub fn comment_body(marker: &Marker, report: &CheckReport, approval_dropped: bool) -> String {
    let mut out = marker.render();
    out.push('\n');
    if approval_dropped {
        out.push('\n');
        out.push_str(APPROVAL_DROPPED);
        out.push('\n');
    }
    out.push('\n');
    let room = MAX_COMMENT_BYTES.saturating_sub(out.len());
    out.push_str(&report.markdown_within(room));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{Block, Section};
    use crate::comment::digest::Sha256;

    fn marker() -> Marker {
        Marker {
            body: Sha256::of(b"b"),
            report: Sha256::of(b"r"),
            main: None,
        }
    }

    fn report(lines: Vec<String>) -> CheckReport {
        CheckReport {
            blocking: true,
            sections: vec![Section {
                title: "Parse problems",
                blocks: vec![Block::Quoted(lines)],
            }],
        }
    }

    #[test]
    fn the_marker_opens_the_comment() {
        let found = report(vec!["x".to_owned()]);
        let text = comment_body(&marker(), &found, false);
        assert_eq!(Marker::of_comment(&text), Some(marker()));
        assert!(text.ends_with(&found.markdown()));
        assert!(!text.contains("Approval removed"));
        let dropped = comment_body(&marker(), &found, true);
        assert_eq!(Marker::of_comment(&dropped), Some(marker()));
        assert!(dropped.contains("**Approval removed.**"));
    }

    #[test]
    fn a_long_report_is_cut_inside_the_cap() {
        let lines = vec!["@someone <!-- ".repeat(40); 400];
        let found = report(lines);
        assert!(found.markdown().len() > MAX_COMMENT_BYTES);
        let text = comment_body(&marker(), &found, true);
        assert!(text.len() <= MAX_COMMENT_BYTES, "{}", text.len());
        assert_eq!(Marker::of_comment(&text), Some(marker()));
        assert!(text.contains("**Report cut.**"));
        let outside = outside_fences(&text);
        assert!(
            !outside.contains('@') && !outside.contains("<!--"),
            "{outside}"
        );
    }

    /// The lines of `text` outside fenced blocks, after the marker line.
    fn outside_fences(text: &str) -> String {
        let mut fence: Option<String> = None;
        let mut out = Vec::new();
        for line in text.lines().skip(1) {
            match &fence {
                Some(open) if line == open => fence = None,
                Some(_) => {}
                None if line.starts_with("```") => {
                    let ticks = line.chars().take_while(|c| *c == '`').count();
                    fence = Some("`".repeat(ticks));
                }
                None => out.push(line),
            }
        }
        assert!(fence.is_none(), "a fence stays open");
        out.join("\n")
    }
}
