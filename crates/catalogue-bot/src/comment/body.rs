//! The text of a bot check comment.

use super::digest::Sha256;
use super::marker::Marker;
use super::status::{Note, PrStatus};
use crate::check::CheckReport;

/// The most bytes a bot comment holds. GitHub refuses a comment past 65,536 characters.
pub const MAX_COMMENT_BYTES: usize = 65_000;

/// The hashed text of a comment: the report, then the PR status when there is one.
///
/// With no status the text is the report alone, so the digest matches a
/// comment written before the PR step.
fn hashed_text(report: &CheckReport, status: Option<&PrStatus>) -> String {
    let mut text = report.markdown();
    if let Some(status) = status {
        text.push('\n');
        text.push_str(&status.markdown());
    }
    text
}

/// The digest the marker records for `report` and the PR `status`.
///
/// A change in the report or the status changes the digest, so the bot
/// rewrites the comment. The notes are not part of it.
pub fn report_digest(report: &CheckReport, status: Option<&PrStatus>) -> Sha256 {
    Sha256::of(hashed_text(report, status).as_bytes())
}

/// The comment for `report`: the marker line, the `notes`, the PR `status`, then the report.
///
/// The text stays within [`MAX_COMMENT_BYTES`]. Only the report is cut, so the
/// marker line is always whole. Issue text appears only inside fenced blocks.
pub fn comment_body(
    marker: &Marker,
    report: &CheckReport,
    status: Option<&PrStatus>,
    notes: &[Note],
) -> String {
    let mut out = marker.render();
    out.push('\n');
    for note in notes {
        out.push('\n');
        out.push_str(&note.markdown());
        out.push('\n');
    }
    if let Some(status) = status {
        out.push('\n');
        out.push_str(&status.markdown());
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
        let text = comment_body(&marker(), &found, None, &[]);
        assert_eq!(Marker::of_comment(&text), Some(marker()));
        assert!(text.ends_with(&found.markdown()));
        assert!(!text.contains("Approval removed"));
        let dropped = comment_body(&marker(), &found, None, &[Note::BodyChanged]);
        assert_eq!(Marker::of_comment(&dropped), Some(marker()));
        assert!(dropped.contains("**Approval removed.**"));
    }

    #[test]
    fn the_status_follows_the_notes_and_enters_the_digest() {
        let found = report(vec!["x".to_owned()]);
        let status = PrStatus::Open(31);
        let text = comment_body(
            &marker(),
            &found,
            Some(&status),
            &[Note::Refused(vec!["r".to_owned()])],
        );
        let note = text.find("**Approval removed.**").expect("note");
        let pr = text.find("**Data PR:** #31 is open").expect("status");
        let check = text.find("## Catalogue check").expect("report");
        assert!(note < pr && pr < check, "{text}");
        assert_eq!(
            report_digest(&found, None),
            Sha256::of(found.markdown().as_bytes())
        );
        assert_ne!(
            report_digest(&found, Some(&status)),
            report_digest(&found, None)
        );
        assert_ne!(
            report_digest(&found, Some(&status)),
            report_digest(&found, Some(&PrStatus::Merged(31)))
        );
    }

    #[test]
    fn a_long_report_is_cut_inside_the_cap() {
        let lines = vec!["@someone <!-- ".repeat(40); 400];
        let found = report(lines);
        assert!(found.markdown().len() > MAX_COMMENT_BYTES);
        let text = comment_body(&marker(), &found, None, &[Note::BodyChanged]);
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
