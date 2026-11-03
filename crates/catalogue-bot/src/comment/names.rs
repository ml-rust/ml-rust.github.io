//! The labels the bot writes on an issue.

/// The label for an issue whose check found no blocking error.
pub const BOT_CHECKED: &str = "bot-checked";
/// The label for an issue whose check found a blocking error.
pub const NEEDS_FIX: &str = "needs-fix";
/// The label a maintainer adds to approve a judgment.
pub const APPROVED: &str = "approved";
/// The label for an issue whose data PR the bot opened.
pub const PR_OPEN: &str = "pr-open";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::parse_labels;

    #[test]
    fn the_labels_file_holds_every_label_the_bot_writes() {
        let file = parse_labels(include_str!("../../../../.github/labels.toml")).expect("parses");
        for name in [BOT_CHECKED, NEEDS_FIX, APPROVED, PR_OPEN] {
            assert!(
                file.labels().iter().any(|label| label.name == name),
                "{name}"
            );
        }
    }
}
