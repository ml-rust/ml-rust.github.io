//! A judgment form into the judgment it proposes.

use std::num::NonZeroU64;

use catalogue_core::{
    GithubLogin, Judgment, JudgmentKind, MaintainerStatus, ProjectKind, Slug, Tier,
};
use url::Url;

use super::meta::{Approval, IssueMeta, JUDGMENTS_DIR};
use crate::form::{
    CORRECTNESS, DEMOTION, FEATURED_APPROVAL, Field, FormError, JudgmentAnswers, JudgmentForm,
    KIND, MAINTAINER, SUPERSEDES, TOPIC, excerpt, label,
};

/// A judgment that waits for approval: every field except the approver and the date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgmentDraft {
    /// The issue number, which names the file.
    pub number: NonZeroU64,
    /// The entry the judgment applies to.
    pub entry: Slug,
    /// The issue link.
    pub issue: Url,
    /// The issue author.
    pub reporter: GithubLogin,
    /// Links that support the decision.
    pub evidence: Vec<Url>,
    /// The decision.
    pub judgment: JudgmentKind,
}

impl JudgmentDraft {
    /// The judgment file path relative to the repository root: `data/judgments/<number>.toml`.
    pub fn path(&self) -> String {
        format!("{JUDGMENTS_DIR}/{}.toml", self.number)
    }

    /// Lists every rule the draft breaks on its own. The approver rules run at approval.
    pub fn problems(&self) -> Vec<String> {
        self.judgment.problems(&self.evidence)
    }

    /// The judgment that `approval` completes.
    pub fn approve(self, approval: Approval) -> Judgment {
        Judgment {
            entry: self.entry,
            issue: self.issue,
            reporter: self.reporter,
            approver: approval.approver,
            date: approval.date,
            evidence: self.evidence,
            judgment: self.judgment,
        }
    }
}

/// The fields of the judgment form that `answers` come from.
fn fields_of(answers: &JudgmentAnswers) -> &'static [Field] {
    match answers {
        JudgmentAnswers::Correctness { .. } => CORRECTNESS,
        JudgmentAnswers::Supersedes { .. } => SUPERSEDES,
        JudgmentAnswers::Kind { .. } => KIND,
        JudgmentAnswers::Topic { .. } => TOPIC,
        JudgmentAnswers::Demotion { .. } => DEMOTION,
        JudgmentAnswers::FeaturedApproval => FEATURED_APPROVAL,
        JudgmentAnswers::Maintainer { .. } => MAINTAINER,
    }
}

fn slug(text: &str) -> Result<Slug, String> {
    Slug::new(text).map_err(|error| error.to_string())
}

fn not_an_option(text: &str) -> String {
    format!("`{}` is not an option", excerpt(text))
}

/// The decision that `answers` record.
fn kind_of(answers: &JudgmentAnswers) -> Result<JudgmentKind, (&'static str, String)> {
    Ok(match answers {
        JudgmentAnswers::Correctness { finding } => JudgmentKind::Correctness {
            finding: finding.clone(),
        },
        JudgmentAnswers::Supersedes { by } => JudgmentKind::Supersedes {
            by: slug(by).map_err(|reason| ("by", reason))?,
        },
        JudgmentAnswers::Kind { value } => JudgmentKind::Kind {
            value: match value.as_str() {
                "product" => ProjectKind::Product,
                "demo" => ProjectKind::Demo,
                other => return Err(("value", not_an_option(other))),
            },
        },
        JudgmentAnswers::Topic { topics } => JudgmentKind::Topic {
            topics: topics
                .iter()
                .map(|topic| slug(topic))
                .collect::<Result<_, _>>()
                .map_err(|reason| ("topics", reason))?,
        },
        JudgmentAnswers::Demotion { to } => JudgmentKind::Demotion {
            from: Tier::Featured,
            to: Tier::from_name(to).ok_or_else(|| ("to", not_an_option(to)))?,
        },
        JudgmentAnswers::FeaturedApproval => JudgmentKind::FeaturedApproval {},
        JudgmentAnswers::Maintainer { status } => JudgmentKind::Maintainer {
            status: match status.as_str() {
                "maintained" => MaintainerStatus::Maintained,
                "finished" => MaintainerStatus::Finished,
                other => return Err(("status", not_an_option(other))),
            },
        },
    })
}

/// Converts the answers of a judgment form into a draft that waits for approval.
///
/// The issue link and the reporter come from `meta`. A demotion always
/// starts from `featured`. Fails with every field problem found.
pub fn judgment_draft(
    form: &JudgmentForm,
    meta: &IssueMeta,
) -> Result<JudgmentDraft, Vec<FormError>> {
    let fields = fields_of(&form.answers);
    let mut problems = Vec::new();
    let invalid = |id: &'static str, reason: String| FormError::Invalid {
        label: label(fields, id),
        reason,
    };
    let entry = slug(&form.entry)
        .map_err(|reason| problems.push(invalid("entry", reason)))
        .ok();
    let judgment = kind_of(&form.answers)
        .map_err(|(id, reason)| problems.push(invalid(id, reason)))
        .ok();
    let mut evidence = Vec::new();
    for text in &form.evidence {
        match Url::parse(text) {
            Ok(url) => evidence.push(url),
            Err(_) => problems.push(invalid(
                "evidence",
                format!("`{}` is not a valid link", excerpt(text)),
            )),
        }
    }
    let issue = meta.issue_url().map_err(|error| problems.push(error)).ok();
    match (entry, judgment, issue) {
        (Some(entry), Some(judgment), Some(issue)) if problems.is_empty() => Ok(JudgmentDraft {
            number: meta.number,
            entry,
            issue,
            reporter: meta.author.clone(),
            evidence,
            judgment,
        }),
        _ => Err(problems),
    }
}

/// Converts the answers of a judgment form into the judgment that `approval` completes.
pub fn judgment(
    form: &JudgmentForm,
    meta: &IssueMeta,
    approval: Approval,
) -> Result<Judgment, Vec<FormError>> {
    judgment_draft(form, meta).map(|draft| draft.approve(approval))
}

#[cfg(test)]
mod tests {
    use catalogue_core::Date;

    use super::*;

    fn meta() -> IssueMeta {
        IssueMeta {
            number: NonZeroU64::new(12).expect("non-zero"),
            author: GithubLogin::new("dave").expect("login"),
        }
    }

    fn form(answers: JudgmentAnswers) -> JudgmentForm {
        JudgmentForm {
            entry: "alpha".to_owned(),
            evidence: vec!["https://example.com/proof".to_owned()],
            answers,
        }
    }

    fn draft(answers: JudgmentAnswers) -> JudgmentDraft {
        judgment_draft(&form(answers), &meta()).expect("converts")
    }

    #[test]
    fn a_demotion_starts_from_featured() {
        let found = draft(JudgmentAnswers::Demotion {
            to: "new".to_owned(),
        });
        assert_eq!(
            found.judgment,
            JudgmentKind::Demotion {
                from: Tier::Featured,
                to: Tier::New
            }
        );
        assert_eq!(found.path(), "data/judgments/12.toml");
        assert_eq!(
            found.issue.as_str(),
            "https://github.com/ml-rust/ml-rust.github.io/issues/12"
        );
        assert!(found.problems().is_empty());
    }

    #[test]
    fn every_kind_converts() {
        let cases = [
            (
                JudgmentAnswers::Correctness {
                    finding: "Wrong.".to_owned(),
                },
                JudgmentKind::Correctness {
                    finding: "Wrong.".to_owned(),
                },
            ),
            (
                JudgmentAnswers::Supersedes {
                    by: "beta".to_owned(),
                },
                JudgmentKind::Supersedes {
                    by: Slug::new("beta").expect("slug"),
                },
            ),
            (
                JudgmentAnswers::Kind {
                    value: "demo".to_owned(),
                },
                JudgmentKind::Kind {
                    value: ProjectKind::Demo,
                },
            ),
            (
                JudgmentAnswers::Topic {
                    topics: vec!["tensors".to_owned()],
                },
                JudgmentKind::Topic {
                    topics: vec![Slug::new("tensors").expect("slug")],
                },
            ),
            (
                JudgmentAnswers::FeaturedApproval,
                JudgmentKind::FeaturedApproval {},
            ),
            (
                JudgmentAnswers::Maintainer {
                    status: "maintained".to_owned(),
                },
                JudgmentKind::Maintainer {
                    status: MaintainerStatus::Maintained,
                },
            ),
        ];
        for (answers, expected) in cases {
            assert_eq!(draft(answers).judgment, expected);
        }
    }

    #[test]
    fn approval_completes_the_judgment() {
        let approval = Approval {
            approver: GithubLogin::new("bob").expect("login"),
            date: Date::new(2026, 11, 4).expect("date"),
        };
        let found = judgment(
            &form(JudgmentAnswers::FeaturedApproval),
            &meta(),
            approval.clone(),
        )
        .expect("converts");
        assert_eq!(found.reporter.as_str(), "dave");
        assert_eq!(found.approver, approval.approver);
        assert_eq!(found.date, approval.date);
        assert!(found.issue_matches(12));
    }

    #[test]
    fn every_bad_field_is_reported() {
        let mut input = form(JudgmentAnswers::Supersedes {
            by: "Not A Slug".to_owned(),
        });
        input.entry = "Alpha".to_owned();
        input.evidence = vec!["nope".to_owned(), "https://ok.example".to_owned()];
        let errors = judgment_draft(&input, &meta()).expect_err("problems");
        let labels: Vec<&str> = errors
            .iter()
            .filter_map(|error| match error {
                FormError::Invalid { label, .. } => Some(*label),
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["Entry slug", "Superseded by", "Evidence links"]);
    }
}
