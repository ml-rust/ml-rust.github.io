//! The claim that the evidence links of one issue must back.

use crate::form::{Form, FormKind, JudgmentAnswers, JudgmentForm, SubmitForm};

/// One named value of a claim. The value is untrusted issue text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimField {
    /// The field name, fixed by the bot.
    pub label: &'static str,
    /// The value as the issue states it.
    pub value: String,
}

/// What an issue claims, and what its evidence must show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The form the claim comes from.
    pub kind: FormKind,
    /// What the evidence must show, fixed by the bot per form.
    pub goal: &'static str,
    /// The claimed values, in a fixed order.
    pub fields: Vec<ClaimField>,
}

impl Claim {
    /// The claim of `form`.
    pub fn of_form(form: &Form) -> Self {
        match form {
            Form::Submit(submit) => submit_claim(submit),
            Form::Judgment(judgment) => judgment_claim(judgment),
        }
    }

    /// The claim as text: the goal, then one `label: value` line per field.
    pub fn text(&self) -> String {
        let mut out = format!(
            "Form: {}\nThe evidence must show: {}\n",
            self.kind.name(),
            self.goal
        );
        for field in &self.fields {
            out.push_str(field.label);
            out.push_str(": ");
            out.push_str(&field.value);
            out.push('\n');
        }
        out
    }
}

fn field(label: &'static str, value: impl Into<String>) -> ClaimField {
    ClaimField {
        label,
        value: value.into(),
    }
}

fn submit_claim(form: &SubmitForm) -> Claim {
    Claim {
        kind: FormKind::Submit,
        goal: "the project exists as described, and people or projects use it.",
        fields: vec![
            field("Name", form.name.as_str()),
            field("Kind", form.kind.as_str()),
            field("Description", form.description.as_str()),
            field("Crates", form.crates.join(", ")),
            field("Repository", form.repo.as_str()),
        ],
    }
}

fn judgment_claim(form: &JudgmentForm) -> Claim {
    let entry = field("Entry", form.entry.as_str());
    let (kind, goal, extra) = match &form.answers {
        JudgmentAnswers::Correctness { finding } => (
            FormKind::Correctness,
            "the finding about the entry is true.",
            Some(field("Finding", finding.as_str())),
        ),
        JudgmentAnswers::Supersedes { by } => (
            FormKind::Supersedes,
            "the entry is replaced by the named entry.",
            Some(field("Replaced by", by.as_str())),
        ),
        JudgmentAnswers::Kind { value } => (
            FormKind::Kind,
            "the entry is of the stated kind: a product or a demo.",
            Some(field("Kind", value.as_str())),
        ),
        JudgmentAnswers::Category { topics } => (
            FormKind::Category,
            "the entry belongs in the stated topics.",
            Some(field("Topics", topics.join(", "))),
        ),
        JudgmentAnswers::Demotion { to } => (
            FormKind::Demotion,
            "the entry no longer meets its tier and belongs in the stated tier.",
            Some(field("Demote to", to.as_str())),
        ),
        JudgmentAnswers::FeaturedApproval => (
            FormKind::FeaturedApproval,
            "the entry is of a quality and use that merit the featured tier.",
            None,
        ),
        JudgmentAnswers::Maintainer { status } => (
            FormKind::Maintainer,
            "the entry has the stated maintenance status: maintained or finished.",
            Some(field("Status", status.as_str())),
        ),
    };
    Claim {
        kind,
        goal,
        fields: std::iter::once(entry).chain(extra).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn submit() -> SubmitForm {
        SubmitForm {
            name: "Gizmo".to_owned(),
            kind: "library".to_owned(),
            repo: "https://github.com/o/gizmo".to_owned(),
            crates: vec!["gizmo".to_owned(), "gizmo-core".to_owned()],
            topics: vec!["inference".to_owned()],
            description: "Runs models.".to_owned(),
            description_source: "human".to_owned(),
            description_source_url: None,
            homepage: None,
            homebrew: None,
            docker: Vec::new(),
            repology: None,
            evidence_of_use: vec!["https://example.com/uses".to_owned()],
            affiliation: "x".to_owned(),
        }
    }

    #[test]
    fn a_submission_claims_its_name_kind_description_crates_and_repo() {
        let claim = Claim::of_form(&Form::Submit(Box::new(submit())));
        assert_eq!(claim.kind, FormKind::Submit);
        assert_eq!(
            claim.text(),
            "Form: submit\nThe evidence must show: the project exists as described, and people or projects use it.\n\
             Name: Gizmo\nKind: library\nDescription: Runs models.\nCrates: gizmo, gizmo-core\n\
             Repository: https://github.com/o/gizmo\n"
        );
    }

    #[test]
    fn each_judgment_claims_its_entry_and_its_own_answer() {
        let cases = [
            (
                JudgmentAnswers::Correctness {
                    finding: "f".to_owned(),
                },
                Some("Finding"),
            ),
            (
                JudgmentAnswers::Supersedes { by: "b".to_owned() },
                Some("Replaced by"),
            ),
            (
                JudgmentAnswers::Kind {
                    value: "demo".to_owned(),
                },
                Some("Kind"),
            ),
            (
                JudgmentAnswers::Category {
                    topics: vec!["a".to_owned(), "b".to_owned()],
                },
                Some("Topics"),
            ),
            (
                JudgmentAnswers::Demotion {
                    to: "listed".to_owned(),
                },
                Some("Demote to"),
            ),
            (JudgmentAnswers::FeaturedApproval, None),
            (
                JudgmentAnswers::Maintainer {
                    status: "finished".to_owned(),
                },
                Some("Status"),
            ),
        ];
        for (answers, label) in cases {
            let form = Form::Judgment(JudgmentForm {
                entry: "gizmo".to_owned(),
                evidence: Vec::new(),
                answers,
            });
            let claim = Claim::of_form(&form);
            assert_eq!(claim.kind, form.kind());
            assert_eq!(claim.fields.first(), Some(&field("Entry", "gizmo")));
            assert_eq!(claim.fields.get(1).map(|found| found.label), label);
        }
    }
}
