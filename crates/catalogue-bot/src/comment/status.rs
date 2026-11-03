//! The data PR status and the one-time notes in a bot comment.

use crate::check::fenced;

/// The note a comment carries when the bot removed `approved` after a body edit.
const BODY_CHANGED: &str = "**Approval removed.** The issue body changed after the last check, \
so the `approved` label no longer covers it. A maintainer must read the new text and approve again.";

/// The lead of the note a comment carries when the approval rules refuse the approver.
const REFUSED: &str = "**Approval removed.** The approval rules refuse the latest `approved` label. \
An eligible maintainer must add it again. The reasons:";

/// The state of the data PR of an issue. It is part of the hashed comment text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrStatus {
    /// The PR with this number is open.
    Open(u64),
    /// The PR with this number is merged.
    Merged(u64),
    /// The PR with this number closed without a merge before the current approval.
    Closed(u64),
    /// The bot opens no PR until these problems are fixed.
    Blocked(Vec<String>),
}

impl PrStatus {
    /// The status as markdown. Problem lines stay inside a fenced block.
    pub fn markdown(&self) -> String {
        match self {
            Self::Open(number) => {
                format!("**Data PR:** #{number} is open. A maintainer reviews and merges it.")
            }
            Self::Merged(number) => format!("**Data PR:** #{number} is merged."),
            Self::Closed(number) => format!(
                "**Data PR:** #{number} closed without a merge. To open a new one, remove the `approved` label and add it again."
            ),
            Self::Blocked(lines) => format!(
                "**Data PR: blocked.** The bot opens no PR until every problem below is fixed.\n\n{}",
                fenced("text", &lines.join("\n"))
            ),
        }
    }
}

/// A note shown once, on the pass that removes `approved`. It is not hashed,
/// so a later pass that writes nothing keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// The body changed after the last check.
    BodyChanged,
    /// The approval rules refuse the approver, for these reasons.
    Refused(Vec<String>),
}

impl Note {
    /// The note as markdown. Reason lines stay inside a fenced block.
    pub fn markdown(&self) -> String {
        match self {
            Self::BodyChanged => BODY_CHANGED.to_owned(),
            Self::Refused(lines) => format!("{REFUSED}\n\n{}", fenced("text", &lines.join("\n"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problem_lines_stay_inside_a_fence() {
        let text = PrStatus::Blocked(vec!["@someone <script>".to_owned()]).markdown();
        assert!(text.ends_with("```text\n@someone <script>\n```"), "{text}");
        let note = Note::Refused(vec!["approver `x` is not a maintainer".to_owned()]).markdown();
        assert!(note.starts_with("**Approval removed.**"), "{note}");
        assert!(note.ends_with("```text\napprover `x` is not a maintainer\n```"));
    }

    #[test]
    fn each_status_names_the_pr() {
        assert!(PrStatus::Open(31).markdown().contains("#31 is open"));
        assert!(PrStatus::Merged(31).markdown().contains("#31 is merged"));
        assert!(PrStatus::Closed(31).markdown().contains("add it again"));
    }
}
