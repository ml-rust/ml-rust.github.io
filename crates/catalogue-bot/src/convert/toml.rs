//! The TOML text of entries, judgments and judgment drafts.

use catalogue_core::{Entry, GithubLogin, Judgment, JudgmentKind, Slug};
use serde::Serialize;
use url::Url;

use super::judgment::JudgmentDraft;

/// The lines that stand in for the approver and the date in a draft.
const PENDING: &str = "# approver: pending. A maintainer approves the issue.\n\
                       # date: pending. Set at approval.\n";

/// The fields of a draft above the approver.
#[derive(Serialize)]
struct Head<'a> {
    entry: &'a Slug,
    issue: &'a Url,
    reporter: &'a GithubLogin,
}

/// The fields of a draft below the date.
#[derive(Serialize)]
struct Tail<'a> {
    evidence: &'a [Url],
    judgment: &'a JudgmentKind,
}

/// The text of `data/entries/<slug>.toml`.
pub fn entry_toml(entry: &Entry) -> Result<String, ::toml::ser::Error> {
    ::toml::to_string(entry)
}

/// The text of `data/judgments/<number>.toml`.
pub fn judgment_toml(judgment: &Judgment) -> Result<String, ::toml::ser::Error> {
    ::toml::to_string(judgment)
}

/// The judgment file text of `draft`, with comment lines in place of the approver and the date.
///
/// The fields keep the order of [`judgment_toml`].
pub fn draft_toml(draft: &JudgmentDraft) -> Result<String, ::toml::ser::Error> {
    let head = ::toml::to_string(&Head {
        entry: &draft.entry,
        issue: &draft.issue,
        reporter: &draft.reporter,
    })?;
    let tail = ::toml::to_string(&Tail {
        evidence: &draft.evidence,
        judgment: &draft.judgment,
    })?;
    Ok(format!("{head}{PENDING}{tail}"))
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use catalogue_core::{
        Date, Description, DescriptionSource, Distribution, EntryKind, MaintainerStatus,
        ProjectKind, Tier,
    };

    use super::*;
    use crate::convert::Approval;

    fn url(text: &str) -> Url {
        Url::parse(text).expect("link")
    }

    fn slug(text: &str) -> Slug {
        Slug::new(text).expect("slug")
    }

    fn login(text: &str) -> GithubLogin {
        GithubLogin::new(text).expect("login")
    }

    fn date() -> Date {
        Date::new(2026, 11, 3).expect("date")
    }

    fn draft(judgment: JudgmentKind) -> JudgmentDraft {
        JudgmentDraft {
            number: NonZeroU64::new(12).expect("non-zero"),
            entry: slug("alpha"),
            issue: url("https://github.com/ml-rust/ml-rust.github.io/issues/12"),
            reporter: login("dave"),
            evidence: vec![url("https://example.com/proof")],
            judgment,
        }
    }

    fn kinds() -> Vec<JudgmentKind> {
        vec![
            JudgmentKind::Correctness {
                finding: "The claim is wrong.\nThe code lacks it.".to_owned(),
            },
            JudgmentKind::Supersedes { by: slug("beta") },
            JudgmentKind::Kind {
                value: ProjectKind::Product,
            },
            JudgmentKind::Category {
                topics: vec![slug("ml"), slug("tensors")],
            },
            JudgmentKind::Demotion {
                from: Tier::Featured,
                to: Tier::Archived,
            },
            JudgmentKind::FeaturedApproval {},
            JudgmentKind::Maintainer {
                status: MaintainerStatus::Finished,
            },
        ]
    }

    #[test]
    fn an_entry_round_trips() {
        let entry = Entry {
            name: "Gizmo \"quoted\"".to_owned(),
            kind: EntryKind::App,
            repo: url("https://github.com/someone/gizmo"),
            homepage: Some(url("https://gizmo.example.com")),
            crates: Vec::new(),
            topics: vec![slug("ml")],
            description: Description {
                text: "Runs models.".to_owned(),
                source: DescriptionSource::Readme,
                source_url: Some(url("https://github.com/someone/gizmo#readme")),
            },
            org_affiliated: false,
            distribution: Some(Distribution {
                homebrew: Some("gizmo".to_owned()),
                docker: vec!["someone/gizmo".to_owned()],
                repology: None,
            }),
            added: date(),
        };
        let text = entry_toml(&entry).expect("serializes");
        assert!(text.contains("added = 2026-11-03"), "{text}");
        let back: Entry = ::toml::from_str(&text).expect("parses back");
        assert_eq!(back, entry);
    }

    #[test]
    fn every_judgment_kind_round_trips() {
        for kind in kinds() {
            let judgment = draft(kind).approve(Approval {
                approver: login("bob"),
                date: date(),
            });
            let text = judgment_toml(&judgment).expect("serializes");
            let back: Judgment = ::toml::from_str(&text).expect("parses back");
            assert_eq!(back, judgment, "{text}");
        }
    }

    #[test]
    fn a_draft_marks_the_approver_and_date_pending() {
        for kind in kinds() {
            let draft = draft(kind);
            let text = draft_toml(&draft).expect("serializes");
            assert!(text.contains(PENDING), "{text}");
            assert!(!text.contains("approver ="), "{text}");
            let filled = text.replace(PENDING, "approver = \"bob\"\ndate = 2026-11-03\n");
            let back: Judgment = ::toml::from_str(&filled).expect("parses once filled");
            let expected = draft.approve(Approval {
                approver: login("bob"),
                date: date(),
            });
            assert_eq!(back, expected, "{text}");
        }
    }
}
