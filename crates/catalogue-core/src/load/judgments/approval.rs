use std::collections::BTreeMap;

use super::super::diagnostics::Diagnostics;
use super::Refs;
use crate::ids::{GithubLogin, Slug};
use crate::judgment::{Judgment, JudgmentKind};

/// Checks the approver against the maintainer list and the conflict rules,
/// and that a featured approval names an org affiliated entry.
pub(super) fn check(path: &str, judgment: &Judgment, refs: &Refs<'_>, diags: &mut Diagnostics) {
    let found = problems(
        &judgment.entry,
        &judgment.judgment,
        &judgment.reporter,
        &judgment.approver,
        refs,
    );
    for message in found {
        diags.push(path, message);
    }
}

/// Lists the rules that `approver` breaks when approving `kind` on `entry` for `reporter`.
pub(super) fn problems(
    entry: &Slug,
    kind: &JudgmentKind,
    reporter: &GithubLogin,
    approver: &GithubLogin,
    refs: &Refs<'_>,
) -> Vec<String> {
    let found = refs.entries.get(entry);
    let affiliated = found.is_some_and(|entry| entry.org_affiliated);
    let subject = Subject {
        entry: entry.as_str(),
        affiliated,
        reporter_can_approve: kind.is_maintainer(),
    };
    let mut out = person_problems(&subject, reporter, approver, refs.maintainers);
    let approves_featured = matches!(kind, JudgmentKind::FeaturedApproval {});
    if found.is_some() && !affiliated && approves_featured {
        out.push(format!(
            "featured-approval applies only to an org affiliated entry, and `{entry}` is not"
        ));
    }
    out
}

/// What the approver rules need to know about the record under approval.
pub(super) struct Subject<'a> {
    /// The entry name shown in messages.
    pub entry: &'a str,
    /// Whether the entry is org affiliated.
    pub affiliated: bool,
    /// Whether the reporter can approve the record.
    pub reporter_can_approve: bool,
}

/// Lists the rules that `approver` breaks on any record: the maintainer list,
/// the reporter rule and the org conflict rule.
///
/// `maintainers` maps a lowercase login to `org_member`.
pub(super) fn person_problems(
    subject: &Subject<'_>,
    reporter: &GithubLogin,
    approver: &GithubLogin,
    maintainers: &BTreeMap<String, bool>,
) -> Vec<String> {
    let mut out = Vec::new();
    let org_member = maintainers.get(&approver.normalized()).copied();
    if org_member.is_none() {
        out.push(format!("approver `{approver}` is not a maintainer"));
    }
    if reporter.same_account(approver) && !subject.reporter_can_approve {
        out.push(format!(
            "approver `{approver}` must differ from the reporter"
        ));
    }
    if subject.affiliated && org_member == Some(true) {
        out.push(format!(
            "entry `{}` is org affiliated, so approver `{approver}` must not be an org member",
            subject.entry
        ));
    }
    out
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

    fn featured_approval(entry: &str, approver: &str) -> String {
        JUDGMENT
            .replace("entry = \"alpha\"", &format!("entry = \"{entry}\""))
            .replace(
                "approver = \"carol\"",
                &format!("approver = \"{approver}\""),
            )
            .replace(
                "type = \"correctness\"\nfinding = \"The README claims support the code lacks.\"\n",
                "type = \"featured-approval\"\n",
            )
    }

    #[test]
    fn outside_approval_of_an_affiliated_entry_passes() {
        let text = featured_approval("beta", "bob");
        assert!(text.contains("featured-approval"), "{text}");
        let mut src = valid();
        put(&mut src, "judgments/7.toml", &text);
        assert!(errors(src).is_empty());
    }

    #[test]
    fn featured_approval_on_an_unaffiliated_entry_is_reported() {
        let mut src = valid();
        put(
            &mut src,
            "judgments/7.toml",
            &featured_approval("alpha", "bob"),
        );
        expect_error(
            src,
            "featured-approval applies only to an org affiliated entry, and `alpha` is not",
        );
    }

    #[test]
    fn org_member_featured_approval_is_reported() {
        let mut src = valid();
        put(
            &mut src,
            "judgments/7.toml",
            &featured_approval("beta", "carol"),
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
