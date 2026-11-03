//! The text of a bot check comment.

use super::digest::Sha256;
use super::marker::{MAX_MARKER_LEN, Marker};
use super::status::{Note, PrStatus};
use crate::check::CheckReport;
use crate::triage::{section_floor, section_within};

/// The most bytes a bot comment holds. GitHub refuses a comment past 65,536 characters.
pub const MAX_COMMENT_BYTES: usize = 65_000;

/// The hashed text of a comment: the report, the PR status when there is
/// one, then the link check section when there is one.
///
/// With no status and no section the text is the report alone, so the digest
/// matches a comment written before the PR step and the link check.
fn hashed_text(report: &CheckReport, status: Option<&PrStatus>, section: Option<&str>) -> String {
    let mut text = report.markdown();
    for part in [status.map(PrStatus::markdown).as_deref(), section]
        .into_iter()
        .flatten()
    {
        text.push('\n');
        text.push_str(part);
    }
    text
}

/// The digest the marker records for `report`, the PR `status` and the link check `section`.
///
/// `section` is the text [`posted_section`] gives. A change in any of them
/// changes the digest, so the bot rewrites the comment. The notes are not part of it.
pub fn report_digest(
    report: &CheckReport,
    status: Option<&PrStatus>,
    section: Option<&str>,
) -> Sha256 {
    Sha256::of(hashed_text(report, status, section).as_bytes())
}

/// The comment text after the marker line, cut to fit.
struct Layout {
    /// The notes, then the PR status, then the blank line before the report.
    head: String,
    /// The report, cut to fit.
    report: String,
    /// The link check section, cut to fit. `None` when the comment shows none.
    section: Option<String>,
}

/// Lays out the text after the marker line within [`MAX_COMMENT_BYTES`].
///
/// The room leaves space for the longest marker line, so the cut never
/// depends on the marker. The section is cut first and the report next. A
/// section keeps at least its summary line and a cut notice.
fn layout(
    report: &CheckReport,
    status: Option<&PrStatus>,
    notes: &[Note],
    section: Option<&str>,
) -> Layout {
    let mut head = String::new();
    for note in notes {
        head.push('\n');
        head.push_str(&note.markdown());
        head.push('\n');
    }
    if let Some(status) = status {
        head.push('\n');
        head.push_str(&status.markdown());
        head.push('\n');
    }
    head.push('\n');
    let used = MAX_MARKER_LEN.saturating_add(1).saturating_add(head.len());
    let room = MAX_COMMENT_BYTES.saturating_sub(used);
    let Some(section) = section else {
        return Layout {
            head,
            report: report.markdown_within(room),
            section: None,
        };
    };
    let report_room = room.saturating_sub(section_floor().saturating_add(1));
    let report = report.markdown_within(report_room);
    let room = room.saturating_sub(report.len().saturating_add(1));
    let section = Some(section_within(section, room)).filter(|text| !text.is_empty());
    Layout {
        head,
        report,
        section,
    }
}

/// The link check `section` as the comment for `report`, `status` and `notes` shows it.
///
/// A section that fits comes back unchanged, and a longer one comes back cut.
/// This is the text [`report_digest`] hashes, and the text the next pass
/// reads back from the comment. A cut section given again comes back
/// unchanged, so a second identical pass keeps the digest.
pub fn posted_section(
    report: &CheckReport,
    status: Option<&PrStatus>,
    notes: &[Note],
    section: Option<&str>,
) -> Option<String> {
    layout(report, status, notes, section).section
}

/// The comment for `report`: the marker line, the `notes`, the PR `status`,
/// the report, then the link check `section`.
///
/// The text stays within [`MAX_COMMENT_BYTES`]. The section is cut first and
/// the report next, so the marker line is always whole. The section shows as
/// [`posted_section`] gives it. Issue text and model text appear only inside
/// fenced blocks.
pub fn comment_body(
    marker: &Marker,
    report: &CheckReport,
    status: Option<&PrStatus>,
    notes: &[Note],
    section: Option<&str>,
) -> String {
    let layout = layout(report, status, notes, section);
    let mut out = marker.render();
    out.push('\n');
    out.push_str(&layout.head);
    out.push_str(&layout.report);
    if let Some(section) = layout.section {
        out.push('\n');
        out.push_str(&section);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{Block, Section};
    use crate::triage::{
        Checked, LinkResult, Outcome, Verdict, section_of_comment, triage_section,
    };

    fn marker() -> Marker {
        Marker {
            body: Sha256::of(b"b"),
            report: Sha256::of(b"r"),
            main: None,
            triage: None,
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
        let text = comment_body(&marker(), &found, None, &[], None);
        assert_eq!(Marker::of_comment(&text), Some(marker()));
        assert!(text.ends_with(&found.markdown()));
        assert!(!text.contains("Approval removed"));
        let dropped = comment_body(&marker(), &found, None, &[Note::BodyChanged], None);
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
            None,
        );
        let note = text.find("**Approval removed.**").expect("note");
        let pr = text.find("**Data PR:** #31 is open").expect("status");
        let check = text.find("## Catalogue check").expect("report");
        assert!(note < pr && pr < check, "{text}");
        assert_eq!(
            report_digest(&found, None, None),
            Sha256::of(found.markdown().as_bytes())
        );
        assert_ne!(
            report_digest(&found, Some(&status), None),
            report_digest(&found, None, None)
        );
        assert_ne!(
            report_digest(&found, Some(&status), None),
            report_digest(&found, Some(&PrStatus::Merged(31)), None)
        );
    }

    #[test]
    fn a_long_report_is_cut_inside_the_cap() {
        let lines = vec!["@someone <!-- ".repeat(40); 400];
        let found = report(lines);
        assert!(found.markdown().len() > MAX_COMMENT_BYTES);
        let text = comment_body(&marker(), &found, None, &[Note::BodyChanged], None);
        assert!(text.len() <= MAX_COMMENT_BYTES, "{}", text.len());
        assert_eq!(Marker::of_comment(&text), Some(marker()));
        assert!(text.contains("**Report cut.**"));
        let outside = outside_fences(&text);
        assert!(
            !outside.contains('@') && !outside.contains("<!--"),
            "{outside}"
        );
    }

    /// A section with `count` links, each with a quote that holds a mention and a comment opener.
    fn section_of(count: usize) -> String {
        let results: Vec<LinkResult> = (0..count)
            .map(|index| LinkResult {
                url: url::Url::parse(&format!("https://example.com/{index}")).expect("url"),
                outcome: Outcome::Checked(Checked {
                    verdict: Verdict::Supports,
                    quote: Some("@someone <!-- used ".repeat(10)),
                }),
            })
            .collect();
        triage_section(&results)
    }

    fn section() -> String {
        section_of(1)
    }

    #[test]
    fn the_section_ends_the_comment_and_enters_the_digest() {
        let found = report(vec!["x".to_owned()]);
        let status = PrStatus::Open(31);
        let section = section();
        let text = comment_body(&marker(), &found, Some(&status), &[], Some(&section));
        let check = text.find("## Catalogue check").expect("report");
        let link = text.find("<details>").expect("section");
        assert!(check < link && text.ends_with(&section), "{text}");
        assert_eq!(section_of_comment(&text), Some(section.clone()));
        assert_eq!(
            comment_body(&marker(), &found, Some(&status), &[], None),
            text.replace(&format!("\n{section}"), "")
        );
        let with = report_digest(&found, Some(&status), Some(&section));
        assert_ne!(with, report_digest(&found, Some(&status), None));
        assert_ne!(with, report_digest(&found, Some(&status), Some("other")));
        assert!(!outside_fences(&text).contains('@'));
    }

    #[test]
    fn a_long_report_cuts_the_section_and_stays_inside_the_cap() {
        let lines = vec!["@someone <!-- ".repeat(40); 400];
        let found = report(lines);
        let section = section_of(60);
        let text = comment_body(&marker(), &found, None, &[], Some(&section));
        assert!(text.len() <= MAX_COMMENT_BYTES, "{}", text.len());
        assert_eq!(Marker::of_comment(&text), Some(marker()));
        assert!(text.contains("**Report cut.**"));
        assert!(text.contains("**Link check cut.**"));
        let kept = section_of_comment(&text).expect("section");
        assert!(kept.len() >= section_floor() && kept.len() < section.len());
        let outside = outside_fences(&text);
        assert!(
            !outside.contains('@') && !outside.contains("<!--"),
            "{outside}"
        );
    }

    #[test]
    fn a_cut_section_read_back_keeps_the_digest_and_the_text() {
        let found = report(vec!["x".to_owned()]);
        let full = section_of(400);
        assert!(full.len() > MAX_COMMENT_BYTES);
        let posted = posted_section(&found, None, &[], Some(&full)).expect("section");
        assert!(posted.len() < full.len() && posted.contains("**Link check cut.**"));
        let first = Marker {
            report: report_digest(&found, None, Some(&posted)),
            ..marker()
        };
        let text = comment_body(&first, &found, None, &[], Some(&full));
        assert!(text.len() <= MAX_COMMENT_BYTES, "{}", text.len());
        let carried = section_of_comment(&text).expect("carried");
        assert_eq!(carried, posted);
        let again = posted_section(&found, None, &[], Some(&carried));
        assert_eq!(again.as_deref(), Some(posted.as_str()));
        let second = Marker {
            report: report_digest(&found, None, again.as_deref()),
            ..marker()
        };
        assert_eq!(second, first);
        assert_eq!(
            comment_body(&second, &found, None, &[], again.as_deref()),
            text
        );
    }

    #[test]
    fn the_cut_does_not_depend_on_the_marker() {
        let lines = vec!["@someone <!-- ".repeat(40); 400];
        let found = report(lines);
        let section = section_of(60);
        let longest = Marker {
            main: Some(catalogue_core::CommitSha::new("a".repeat(40)).expect("sha")),
            triage: Some(Sha256::of(b"t")),
            ..marker()
        };
        let short = comment_body(&marker(), &found, None, &[], Some(&section));
        let long = comment_body(&longest, &found, None, &[], Some(&section));
        assert!(long.len() <= MAX_COMMENT_BYTES, "{}", long.len());
        assert_eq!(section_of_comment(&short), section_of_comment(&long));
        assert_eq!(
            section_of_comment(&long),
            posted_section(&found, None, &[], Some(&section))
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
