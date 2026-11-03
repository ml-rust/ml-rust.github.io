use super::super::diagnostics::Diagnostics;
use super::Refs;
use crate::ids::Slug;
use crate::judgment::JudgmentKind;

/// Checks that the entry, replacement entry and topics of a `kind` judgment on `entry` exist.
pub(super) fn check(
    path: &str,
    entry: &Slug,
    kind: &JudgmentKind,
    refs: &Refs<'_>,
    diags: &mut Diagnostics,
) {
    if !refs.entries.contains_key(entry) {
        diags.push(path, format!("entry `{entry}` does not exist"));
    }
    match kind {
        JudgmentKind::Supersedes { by } => {
            if by == entry {
                diags.push(path, format!("entry `{by}` cannot supersede itself"));
            } else if !refs.entries.contains_key(by) {
                diags.push(path, format!("replacing entry `{by}` does not exist"));
            }
        }
        JudgmentKind::Category { topics } => {
            for topic in topics {
                if !refs.topics.contains(topic) {
                    diags.push(path, format!("topic `{topic}` does not exist"));
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{
        JUDGMENT, category_judgment as category, errors, expect_error, put, supersedes_judgment,
        valid,
    };

    fn supersedes(by: &str) -> String {
        supersedes_judgment(7, "alpha", by)
    }

    #[test]
    fn existing_references_pass() {
        let mut src = valid();
        put(&mut src, "judgments/7.toml", &category("[\"tensors\"]"));
        put(
            &mut src,
            "judgments/8.toml",
            &supersedes_judgment(8, "alpha", "beta"),
        );
        assert!(errors(src).is_empty());
    }

    #[test]
    fn entry_must_exist() {
        let mut src = valid();
        put(
            &mut src,
            "judgments/7.toml",
            &JUDGMENT.replace("entry = \"alpha\"", "entry = \"ghost\""),
        );
        expect_error(src, "entry `ghost` does not exist");
    }

    #[test]
    fn replacing_entry_must_exist() {
        let mut src = valid();
        put(&mut src, "judgments/7.toml", &supersedes("ghost"));
        expect_error(src, "replacing entry `ghost` does not exist");
    }

    #[test]
    fn an_entry_cannot_supersede_itself() {
        let mut src = valid();
        put(&mut src, "judgments/7.toml", &supersedes("alpha"));
        expect_error(src, "cannot supersede itself");
    }

    #[test]
    fn category_topics_must_exist() {
        let mut src = valid();
        put(&mut src, "judgments/7.toml", &category("[\"ghost\"]"));
        expect_error(src, "topic `ghost` does not exist");
    }
}
