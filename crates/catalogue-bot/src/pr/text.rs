//! The branch, title, commit message and body of a data PR, from typed values only.

use std::num::NonZeroU64;

use catalogue_core::{GithubLogin, Slug};
use url::Url;

use super::approval::ApprovalEvent;
use super::ownership::Basis;
use crate::comment::Sha256;
use crate::convert::Affiliation;
use crate::form::FormKind;

/// The prefix of every bot branch. The bot owns the `bot/` namespace.
pub const BRANCH_PREFIX: &str = "bot/issue-";

/// The bot branch of issue `number`: `bot/issue-<number>`.
pub fn branch_name(number: NonZeroU64) -> String {
    format!("{BRANCH_PREFIX}{number}")
}

/// What the PR records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Subject<'a> {
    /// A new entry with this slug and the reporter's affiliation answer.
    Entry {
        /// The entry slug.
        slug: &'a Slug,
        /// The affiliation answer.
        affiliation: Affiliation,
    },
    /// A judgment of this form on this entry.
    Judgment {
        /// The judgment form.
        kind: FormKind,
        /// The entry it applies to.
        entry: &'a Slug,
    },
}

/// Every typed value the PR text holds.
#[derive(Debug, Clone)]
pub(super) struct PrFacts<'a> {
    /// The issue number.
    pub number: NonZeroU64,
    /// The issue link.
    pub issue: &'a Url,
    /// The issue author.
    pub reporter: &'a GithubLogin,
    /// The approval.
    pub approval: &'a ApprovalEvent,
    /// The digest of the issue body the bot checked.
    pub body: Sha256,
    /// The file path relative to the repository root.
    pub path: &'a str,
    /// What the PR records.
    pub subject: Subject<'a>,
    /// The evidence links.
    pub evidence: &'a [Url],
    /// The ownership basis of a maintainer statement.
    pub ownership: Option<&'a Basis>,
}

/// The PR title, which is also the commit subject.
pub(super) fn title(subject: &Subject<'_>) -> String {
    match subject {
        Subject::Entry { slug, .. } => format!("data: add {slug}"),
        Subject::Judgment { kind, entry } => {
            format!("data: {} judgment for {entry}", kind.name())
        }
    }
}

/// The commit message: the title, a blank line and the issue number.
pub(super) fn commit_message(facts: &PrFacts<'_>) -> String {
    format!(
        "{}\n\nFrom issue #{}.\n",
        title(&facts.subject),
        facts.number
    )
}

/// One evidence link as a list item. Only an http or https link renders as a
/// link. A link with another scheme renders as a note.
fn evidence_line(link: &Url) -> String {
    match link.scheme() {
        "http" | "https" => format!("- <{link}>"),
        _ => "- A link with another scheme. It is not shown. See the issue.".to_owned(),
    }
}

/// The PR body. It holds bot text, logins in inline code, and parsed links.
///
/// No free-form issue text enters it. Logins never form a mention.
pub(super) fn body(facts: &PrFacts<'_>) -> String {
    let number = facts.number;
    let lead = match &facts.subject {
        Subject::Entry { .. } => format!(
            "This PR adds `{}`, a new entry from issue #{number}.",
            facts.path
        ),
        Subject::Judgment { kind, entry } => format!(
            "This PR adds `{}`, a `{}` judgment for entry `{entry}` from issue #{number}.",
            facts.path,
            kind.name()
        ),
    };
    let mut lines = vec![
        lead,
        String::new(),
        format!("- Issue: <{}>", facts.issue),
        format!("- Reporter: `{}`", facts.reporter),
        format!(
            "- Approver: `{}`, on {}",
            facts.approval.approver, facts.approval.date
        ),
        format!("- Issue body SHA-256: `{}`", facts.body),
        format!("- File: `{}`", facts.path),
    ];
    if let Subject::Entry { affiliation, .. } = &facts.subject {
        lines.push(format!("- Affiliation: {}", affiliation.sentence()));
    }
    if let Some(basis) = facts.ownership {
        lines.push(format!("- Ownership: {}", basis.sentence(facts.reporter)));
    }
    lines.push(String::new());
    let heading = match facts.subject {
        Subject::Entry { .. } => "Evidence of use",
        Subject::Judgment { .. } => "Evidence",
    };
    if facts.evidence.is_empty() {
        lines.push(format!("{heading}: none."));
    } else {
        lines.push(format!("{heading}:"));
        lines.push(String::new());
        lines.extend(facts.evidence.iter().map(evidence_line));
    }
    lines.push(String::new());
    lines.push(format!("Closes #{number}"));
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use catalogue_core::{CrateName, Date};

    use super::*;

    fn number() -> NonZeroU64 {
        NonZeroU64::new(12).expect("non-zero")
    }

    fn approval() -> ApprovalEvent {
        ApprovalEvent {
            approver: GithubLogin::new("bob").expect("login"),
            at: "2026-11-04T09:00:00Z".parse().expect("time"),
            date: Date::new(2026, 11, 4).expect("date"),
        }
    }

    #[test]
    fn the_branch_comes_from_the_number() {
        assert_eq!(branch_name(number()), "bot/issue-12");
    }

    #[test]
    fn titles_come_from_validated_fields() {
        let slug = Slug::new("gizmo").expect("slug");
        assert_eq!(
            title(&Subject::Entry {
                slug: &slug,
                affiliation: Affiliation::NotAffiliated
            }),
            "data: add gizmo"
        );
        assert_eq!(
            title(&Subject::Judgment {
                kind: FormKind::FeaturedApproval,
                entry: &slug
            }),
            "data: featured-approval judgment for gizmo"
        );
    }

    #[test]
    fn the_body_holds_typed_values_only() {
        let slug = Slug::new("alpha").expect("slug");
        let issue =
            Url::parse("https://github.com/ml-rust/ml-rust.github.io/issues/12").expect("link");
        let reporter = GithubLogin::new("dave").expect("login");
        let approval = approval();
        let evidence = vec![
            Url::parse("https://evil.example/<script>alert(1)</script>").expect("link"),
            Url::parse("https://example.com/@user [x](y)").expect("link"),
            Url::parse("javascript:alert(1)").expect("link"),
        ];
        let basis = Basis::CrateOwner(CrateName::new("alpha-crate").expect("crate"));
        let facts = PrFacts {
            number: number(),
            issue: &issue,
            reporter: &reporter,
            approval: &approval,
            body: Sha256::of(b"body"),
            path: "data/judgments/12.toml",
            subject: Subject::Judgment {
                kind: FormKind::Maintainer,
                entry: &slug,
            },
            evidence: &evidence,
            ownership: Some(&basis),
        };
        let text = body(&facts);
        assert!(text.ends_with("\nCloses #12\n"), "{text}");
        assert!(text.contains("- Reporter: `dave`\n"), "{text}");
        assert!(
            text.contains("- Approver: `bob`, on 2026-11-04\n"),
            "{text}"
        );
        assert!(text.contains("- Ownership: `dave` owns crate `alpha-crate` on crates.io."));
        assert!(
            text.contains(&format!("`{}`", Sha256::of(b"body"))),
            "{text}"
        );
        assert!(!text.contains("<script>"), "{text}");
        assert!(!text.contains("javascript:"), "{text}");
        assert!(!text.contains(" @"), "{text}");
        assert!(
            text.contains("- <https://example.com/@user%20[x](y)>"),
            "{text}"
        );
        assert!(commit_message(&facts).starts_with("data: maintainer judgment for alpha\n\n"));
    }
}
