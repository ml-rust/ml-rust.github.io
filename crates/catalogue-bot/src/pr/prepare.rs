//! The rules that need the fresh issue and `main`, and the PR an issue becomes.

use catalogue_core::{Catalogue, DataPlace, Entry, Judgment, SourceFile};

use super::approval::ApprovalEvent;
use super::decision::{PrDecision, PrInput, PrPlan, precheck};
use super::ownership::{Basis, Ownership};
use super::text::{PrFacts, Subject, body, branch_name, commit_message, title};
use crate::check::{Record, approver_listing, proposed_record, sorted};
use crate::comment::Sha256;
use crate::convert::{IssueMeta, Submission, entry_toml, judgment_toml};
use crate::form::{FormKind, parse_form};

/// Why the bot opens no PR when the body changed during the pass.
const BODY_DRIFT: &str =
    "the issue body changed after this pass checked it. The next pass checks the new text";
/// Why the bot opens no PR for a maintainer statement without an ownership result.
const OWNERSHIP_NOT_RUN: &str = "the ownership check did not run. The next pass runs it again";

/// The approved record.
#[derive(Debug, Clone)]
enum Approved {
    /// A new entry.
    Submit(Box<Submission>),
    /// A judgment of this form, with the approver and the date set.
    Judgment(FormKind, Box<Judgment>),
}

/// An issue that passed every rule except ownership.
#[derive(Debug, Clone)]
pub struct Prepared {
    meta: IssueMeta,
    approval: ApprovalEvent,
    body: Sha256,
    path: String,
    contents: String,
    record: Approved,
    owner_entry: Option<Entry>,
}

/// Runs every rule except ownership on `input`.
///
/// After [`precheck`]: the fresh body must match the checked one, the form
/// must convert, the file must be new on `main`, the approver must pass the
/// approval rules, and `main` with the new file must load without a diagnostic.
///
/// # Errors
///
/// Returns the decision that stops the PR.
pub fn prepare(input: &PrInput<'_>) -> Result<Prepared, PrDecision> {
    let issue = input.issue;
    let (kind, event) = precheck(issue.open, issue.labels, input.blocking, input.approval)?;
    let body = Sha256::of(issue.body.as_bytes());
    if body != input.checked_body {
        return Err(PrDecision::Blocked(vec![BODY_DRIFT.to_owned()]));
    }
    let form = parse_form(kind, issue.body);
    let (record, path) = match proposed_record(&form, kind, input.meta, input.today) {
        (Some(proposed), _) => {
            let path = proposed.path();
            let record = match proposed {
                Record::Submit(found) => Approved::Submit(found),
                Record::Judgment(draft) => {
                    Approved::Judgment(kind, Box::new(draft.approve(event.approval())))
                }
            };
            (record, path)
        }
        (None, problems) => return Err(PrDecision::Blocked(problems)),
    };
    let (place, contents) = file(&record, &path)?;
    let on_main = input
        .main
        .entries
        .iter()
        .chain(&input.main.judgments)
        .any(|found| found.path == path);
    if on_main {
        return Err(PrDecision::Blocked(vec![format!(
            "`{path}` already exists on main"
        )]));
    }
    let mut sources = input.main.clone();
    sources.put(
        place,
        SourceFile {
            path: path.clone(),
            contents: contents.clone(),
        },
    );
    let loaded = Catalogue::load(sources);
    let refused = match &loaded {
        Ok(catalogue) => refusals(catalogue, &record, event),
        Err(_) => Catalogue::load(input.main.clone())
            .map(|catalogue| refusals(&catalogue, &record, event))
            .unwrap_or_default(),
    };
    if !refused.is_empty() {
        return Err(PrDecision::RemoveApproval(refused));
    }
    let catalogue = loaded.map_err(|diagnostics| {
        PrDecision::Blocked(sorted(
            diagnostics.iter().map(ToString::to_string).collect(),
        ))
    })?;
    let owner_entry = match &record {
        Approved::Judgment(FormKind::Maintainer, judgment) => {
            catalogue.entry(&judgment.entry).cloned()
        }
        _ => None,
    };
    Ok(Prepared {
        meta: input.meta.clone(),
        approval: event.clone(),
        body,
        path,
        contents,
        record,
        owner_entry,
    })
}

/// The place and text of the file `record` adds at `path`.
fn file(record: &Approved, path: &str) -> Result<(DataPlace, String), PrDecision> {
    let (place, text) = match record {
        Approved::Submit(found) => (DataPlace::Entries, entry_toml(&found.entry)),
        Approved::Judgment(_, judgment) => (DataPlace::Judgments, judgment_toml(judgment)),
    };
    let text = text.map_err(|error| {
        PrDecision::Blocked(vec![format!(
            "the file `{path}` does not serialize: {error}"
        )])
    })?;
    Ok((place, text))
}

/// The approval rules that `event` breaks on `record`, against `catalogue`.
fn refusals(catalogue: &Catalogue, record: &Approved, event: &ApprovalEvent) -> Vec<String> {
    match record {
        Approved::Submit(found) => {
            let approvers = catalogue.submission_approvers(
                found.slug.as_str(),
                found.entry.org_affiliated,
                &found.notes.reporter,
            );
            if approvers
                .iter()
                .any(|login| login.same_account(&event.approver))
            {
                return Vec::new();
            }
            vec![format!(
                "approver `{}` cannot approve this submission. {}",
                event.approver,
                approver_listing(approvers)
            )]
        }
        Approved::Judgment(_, judgment) => catalogue.approval_problems(judgment),
    }
}

impl Prepared {
    /// The entry on `main` whose ownership a `judgment:maintainer` issue needs. `None` for any other form.
    pub fn owner_entry(&self) -> Option<&Entry> {
        self.owner_entry.as_ref()
    }

    /// The decision once the ownership rules ran. `ownership` matters only for
    /// `judgment:maintainer`, where an unconfirmed result blocks.
    pub fn finish(self, ownership: Option<&Ownership>) -> PrDecision {
        let basis: Option<&Basis> = match (&self.record, ownership) {
            (Approved::Judgment(FormKind::Maintainer, _), Some(Ownership::Confirmed(basis))) => {
                Some(basis)
            }
            (Approved::Judgment(FormKind::Maintainer, _), Some(Ownership::Unconfirmed(lines))) => {
                return PrDecision::Blocked(lines.clone());
            }
            (Approved::Judgment(FormKind::Maintainer, _), None) => {
                return PrDecision::Blocked(vec![OWNERSHIP_NOT_RUN.to_owned()]);
            }
            _ => None,
        };
        let (subject, issue, reporter, evidence) = match &self.record {
            Approved::Submit(found) => (
                Subject::Entry {
                    slug: &found.slug,
                    affiliation: found.notes.affiliation,
                },
                &found.notes.issue,
                &found.notes.reporter,
                found.notes.evidence_of_use.as_slice(),
            ),
            Approved::Judgment(kind, judgment) => (
                Subject::Judgment {
                    kind: *kind,
                    entry: &judgment.entry,
                },
                &judgment.issue,
                &judgment.reporter,
                judgment.evidence.as_slice(),
            ),
        };
        let facts = PrFacts {
            number: self.meta.number,
            issue,
            reporter,
            approval: &self.approval,
            body: self.body,
            path: &self.path,
            subject,
            evidence,
            ownership: basis,
        };
        let title = title(&facts.subject);
        let commit_message = commit_message(&facts);
        let body = body(&facts);
        PrDecision::Open(Box::new(PrPlan {
            branch: branch_name(self.meta.number),
            title,
            commit_message,
            body,
            path: self.path,
            contents: self.contents,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use catalogue_core::{CrateName, DataSources, Date, GithubLogin};

    use super::*;
    use crate::pr::approval::ApprovalSource;
    use crate::pr::decision::{PrIssue, pr_decision};

    const ALPHA: &str = r#"name = "Alpha"
kind = "library"
repo = "https://github.com/ml-rust/alpha"
crates = ["alpha-crate"]
topics = ["ml"]
added = 2026-11-03

[description]
text = "The Alpha project."
source = "human"
"#;

    const TOPICS: &str = r#"[[topic]]
id = "ml"
name = "Machine learning"
"#;

    const MAINTAINERS: &str = r#"[[maintainer]]
login = "carol"
org_member = true
since = 2026-01-01

[[maintainer]]
login = "bob"
org_member = false
since = 2026-02-01
"#;

    const SUBMIT: &str = include_str!("../../tests/fixtures/submit.md");
    const MAINTAINER: &str = include_str!("../../tests/fixtures/maintainer.md");

    fn file(path: &str, contents: &str) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            contents: contents.to_owned(),
        }
    }

    fn main_data() -> DataSources {
        DataSources {
            entries: vec![file("data/entries/alpha.toml", ALPHA)],
            topics: Some(file("data/topics.toml", TOPICS)),
            maintainers: Some(file("data/maintainers.toml", MAINTAINERS)),
            ..DataSources::default()
        }
    }

    fn meta(author: &str) -> IssueMeta {
        IssueMeta {
            number: NonZeroU64::new(12).expect("non-zero"),
            author: GithubLogin::new(author).expect("login"),
        }
    }

    fn approved_by(login: &str) -> ApprovalSource {
        ApprovalSource::Approved(ApprovalEvent {
            approver: GithubLogin::new(login).expect("login"),
            at: "2026-11-04T09:00:00Z".parse().expect("time"),
            date: Date::new(2026, 11, 4).expect("date"),
        })
    }

    fn labels(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    /// The decision on issue 12 by `author` with `body` and `labels`.
    struct Case {
        author: &'static str,
        body: String,
        checked: String,
        labels: Vec<String>,
        open: bool,
        blocking: bool,
        approval: ApprovalSource,
        main: DataSources,
    }

    impl Case {
        fn submit() -> Self {
            Self {
                author: "dave",
                body: SUBMIT.to_owned(),
                checked: SUBMIT.to_owned(),
                labels: labels(&["submit", "approved"]),
                open: true,
                blocking: false,
                approval: approved_by("bob"),
                main: main_data(),
            }
        }

        fn maintainer() -> Self {
            Self {
                body: MAINTAINER.to_owned(),
                checked: MAINTAINER.to_owned(),
                labels: labels(&["judgment:maintainer", "approved"]),
                ..Self::submit()
            }
        }

        fn decide(&self, ownership: Option<&Ownership>) -> PrDecision {
            let meta = meta(self.author);
            pr_decision(
                &PrInput {
                    meta: &meta,
                    issue: PrIssue {
                        open: self.open,
                        labels: &self.labels,
                        body: &self.body,
                    },
                    blocking: self.blocking,
                    checked_body: Sha256::of(self.checked.as_bytes()),
                    approval: &self.approval,
                    main: &self.main,
                    today: Date::new(2026, 11, 4).expect("date"),
                },
                ownership,
            )
        }
    }

    fn plan(decision: PrDecision) -> PrPlan {
        match decision {
            PrDecision::Open(plan) => *plan,
            other => panic!("not open: {other:?}"),
        }
    }

    #[test]
    fn an_approved_submission_opens_a_pr() {
        let found = plan(Case::submit().decide(None));
        assert_eq!(found.branch, "bot/issue-12");
        assert_eq!(found.path, "data/entries/gizmo.toml");
        assert_eq!(found.title, "data: add gizmo");
        assert!(
            found.contents.contains("added = 2026-11-04"),
            "{}",
            found.contents
        );
        assert!(
            found.body.contains("- Approver: `bob`, on 2026-11-04"),
            "{}",
            found.body
        );
        assert!(found.body.contains("- Affiliation: I am not affiliated"));
        assert!(found.body.contains("- <https://example.com/uses-gizmo>"));
        assert!(found.body.ends_with("Closes #12\n"));
        assert!(
            !found.body.contains("A gizmo that learns"),
            "{}",
            found.body
        );
    }

    #[test]
    fn body_drift_blocks() {
        let mut drift = Case::submit();
        drift.body = SUBMIT.replace("A gizmo that learns", "A gizmo that trains");
        assert_eq!(
            drift.decide(None),
            PrDecision::Blocked(vec![BODY_DRIFT.to_owned()])
        );
    }

    #[test]
    fn the_approval_rules_refuse_the_reporter_and_an_org_member() {
        let mut own = Case::submit();
        own.author = "bob";
        let PrDecision::RemoveApproval(lines) = own.decide(None) else {
            panic!("refused");
        };
        assert_eq!(
            lines,
            ["approver `bob` cannot approve this submission. Eligible approvers: `carol`."]
        );
        let mut org = Case::submit();
        org.body = SUBMIT.replace("someone/gizmo", "ml-rust/gizmo");
        org.checked = org.body.clone();
        org.approval = approved_by("carol");
        assert!(matches!(org.decide(None), PrDecision::RemoveApproval(_)));
        let mut stranger = Case::maintainer();
        stranger.approval = approved_by("zed");
        let PrDecision::RemoveApproval(lines) = stranger.decide(None) else {
            panic!("refused");
        };
        assert_eq!(lines, ["approver `zed` is not a maintainer"]);
    }

    #[test]
    fn a_load_diagnostic_or_an_existing_file_blocks() {
        let mut topic = Case::submit();
        topic.body = SUBMIT.replace("\nml\n", "\nnope\n");
        topic.checked = topic.body.clone();
        let PrDecision::Blocked(lines) = topic.decide(None) else {
            panic!("blocked");
        };
        assert!(lines.iter().any(|line| line.contains("nope")), "{lines:?}");
        let mut taken = Case::submit();
        taken
            .main
            .entries
            .push(file("data/entries/gizmo.toml", ALPHA));
        assert_eq!(
            taken.decide(None),
            PrDecision::Blocked(vec![
                "`data/entries/gizmo.toml` already exists on main".to_owned()
            ])
        );
    }

    #[test]
    fn a_maintainer_statement_needs_confirmed_ownership() {
        let case = Case::maintainer();
        let meta = meta("dave");
        let prepared = prepare(&PrInput {
            meta: &meta,
            issue: PrIssue {
                open: true,
                labels: &case.labels,
                body: &case.body,
            },
            blocking: false,
            checked_body: Sha256::of(case.body.as_bytes()),
            approval: &case.approval,
            main: &case.main,
            today: Date::new(2026, 11, 4).expect("date"),
        })
        .expect("prepared");
        let entry = prepared.owner_entry().expect("owner entry");
        assert_eq!(entry.crates[0].as_str(), "alpha-crate");
        assert_eq!(
            case.decide(None),
            PrDecision::Blocked(vec![OWNERSHIP_NOT_RUN.to_owned()])
        );
        let unconfirmed = Ownership::Unconfirmed(vec!["checked".to_owned()]);
        assert_eq!(
            case.decide(Some(&unconfirmed)),
            PrDecision::Blocked(vec!["checked".to_owned()])
        );
        let confirmed = Ownership::Confirmed(Basis::CrateOwner(
            CrateName::new("alpha-crate").expect("crate"),
        ));
        let found = plan(case.decide(Some(&confirmed)));
        assert_eq!(found.path, "data/judgments/12.toml");
        assert_eq!(found.title, "data: maintainer judgment for alpha");
        assert!(
            found.contents.contains("approver = \"bob\""),
            "{}",
            found.contents
        );
        assert!(
            found.contents.contains("date = 2026-11-04"),
            "{}",
            found.contents
        );
        assert!(
            found
                .body
                .contains("- Ownership: `dave` owns crate `alpha-crate` on crates.io.")
        );
    }

    #[test]
    fn injected_issue_text_never_reaches_the_body() {
        let mut hostile = Case::submit();
        hostile.body = SUBMIT
            .replace(
                "A gizmo that learns\nfrom data.",
                "@everyone <script>x</script> [click](https://evil.example)",
            )
            .replace("Gizmo\n", "Gizmo @admin <img src=x>\n");
        hostile.checked = hostile.body.clone();
        let found = plan(hostile.decide(None));
        for needle in ["@everyone", "@admin", "<script>", "<img", "[click]"] {
            assert!(!found.body.contains(needle), "{needle}: {}", found.body);
            assert!(!found.title.contains(needle), "{needle}: {}", found.title);
            assert!(!found.commit_message.contains(needle), "{needle}");
        }
    }
}
