use super::super::diagnostics::Diagnostics;
use super::Refs;
use crate::judgment::Judgment;

/// Checks the approver against the maintainer list and the conflict rules.
pub(super) fn check(path: &str, judgment: &Judgment, refs: &Refs<'_>, diags: &mut Diagnostics) {
    let org_member = refs
        .maintainers
        .get(&judgment.approver.normalized())
        .copied();
    if org_member.is_none() {
        diags.push(
            path,
            format!("approver `{}` is not a maintainer", judgment.approver),
        );
    }
    if judgment.reporter.same_account(&judgment.approver) && !judgment.judgment.is_maintainer() {
        diags.push(
            path,
            format!(
                "approver `{}` must differ from the reporter",
                judgment.approver
            ),
        );
    }
    let affiliated = refs
        .entries
        .get(&judgment.entry)
        .is_some_and(|entry| entry.org_affiliated);
    if affiliated && org_member == Some(true) {
        diags.push(
            path,
            format!(
                "entry `{}` is org affiliated, so approver `{}` must not be an org member",
                judgment.entry, judgment.approver
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{JUDGMENT, errors, expect_error, put, valid};

    const MAINTAINER_SELF: &str = r#"entry = "alpha"
issue = "https://github.com/ml-rust/ml-rust.github.io/issues/7"
reporter = "bob"
approver = "bob"
date = 2026-11-03

[judgment]
type = "maintainer"
status = "finished"
"#;

    #[test]
    fn listed_approver_passes() {
        assert!(errors(valid()).is_empty());
    }

    #[test]
    fn approver_must_be_a_maintainer() {
        let mut src = valid();
        put(
            &mut src,
            "judgments/7.toml",
            &JUDGMENT.replace("approver = \"carol\"", "approver = \"zed\""),
        );
        expect_error(src, "approver `zed` is not a maintainer");
    }

    #[test]
    fn approver_must_differ_from_reporter() {
        let mut src = valid();
        put(
            &mut src,
            "judgments/7.toml",
            &JUDGMENT.replace("reporter = \"dave\"", "reporter = \"carol\""),
        );
        expect_error(src, "must differ from the reporter");
    }

    #[test]
    fn maintainer_judgment_allows_the_owner_to_approve_it() {
        let mut src = valid();
        put(&mut src, "judgments/7.toml", MAINTAINER_SELF);
        assert!(errors(src).is_empty());
    }

    #[test]
    fn org_members_cannot_approve_affiliated_entries() {
        let mut src = valid();
        put(
            &mut src,
            "judgments/7.toml",
            &JUDGMENT.replace("entry = \"alpha\"", "entry = \"beta\""),
        );
        expect_error(src, "must not be an org member");
    }

    #[test]
    fn non_members_can_approve_affiliated_entries() {
        let mut src = valid();
        let text = JUDGMENT
            .replace("entry = \"alpha\"", "entry = \"beta\"")
            .replace("approver = \"carol\"", "approver = \"bob\"");
        put(&mut src, "judgments/7.toml", &text);
        assert!(errors(src).is_empty());
    }
}
