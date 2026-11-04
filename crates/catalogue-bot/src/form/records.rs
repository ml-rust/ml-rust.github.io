//! The answers of each form as named fields, still as text.

use super::answer::Answer;
use super::error::FormError;
use super::fields::Field;
use super::kind::FormKind;

/// The answers of the `submit` form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitForm {
    /// `name`: the display name.
    pub name: String,
    /// `kind`: `library` or `app`.
    pub kind: String,
    /// `repo`: the repository link as written.
    pub repo: String,
    /// `crates`: crate names, one per item.
    pub crates: Vec<String>,
    /// `topics`: topic slugs, one per item.
    pub topics: Vec<String>,
    /// `category`: the category id.
    pub category: Option<String>,
    /// `description`: the text as written.
    pub description: String,
    /// `description_source`: `readme` or `human`.
    pub description_source: String,
    /// `description_source_url`: the page the text comes from.
    pub description_source_url: Option<String>,
    /// `homepage`: the project home page.
    pub homepage: Option<String>,
    /// `homebrew`: the Homebrew formula name.
    pub homebrew: Option<String>,
    /// `docker`: container image names, one per item.
    pub docker: Vec<String>,
    /// `repology`: the Repology project name.
    pub repology: Option<String>,
    /// `evidence_of_use`: links, one per item.
    pub evidence_of_use: Vec<String>,
    /// `affiliation`: one of the two affiliation sentences.
    pub affiliation: String,
}

/// The answers of a judgment form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgmentForm {
    /// `entry`: the slug of the entry the judgment applies to.
    pub entry: String,
    /// `evidence`: links, one per item. Empty when the optional field is empty.
    pub evidence: Vec<String>,
    /// The answers that differ per form.
    pub answers: JudgmentAnswers,
}

/// The answers that differ per judgment form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JudgmentAnswers {
    /// `judgment:correctness`.
    Correctness {
        /// `finding`: what is wrong and what is correct.
        finding: String,
    },
    /// `judgment:supersedes`.
    Supersedes {
        /// `by`: the slug of the replacing entry.
        by: String,
    },
    /// `judgment:kind`.
    Kind {
        /// `value`: `product` or `demo`.
        value: String,
    },
    /// `judgment:topic`.
    Topic {
        /// `topics`: topic slugs, one per item.
        topics: Vec<String>,
    },
    /// `judgment:demotion`.
    Demotion {
        /// `to`: `listed`, `new` or `archived`.
        to: String,
    },
    /// `judgment:featured-approval`. The form has no other field.
    FeaturedApproval,
    /// `judgment:maintainer`.
    Maintainer {
        /// `status`: `maintained` or `finished`.
        status: String,
    },
}

/// The answers of one issue form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Form {
    /// A project submission.
    Submit(Box<SubmitForm>),
    /// A judgment request.
    Judgment(JudgmentForm),
}

/// The answers of one form, looked up by field id.
pub(super) struct Answers {
    fields: &'static [Field],
    answers: Vec<Answer>,
}

impl Answers {
    /// Pairs `answers` with `fields` in order.
    pub(super) fn new(fields: &'static [Field], answers: Vec<Answer>) -> Self {
        Self { fields, answers }
    }

    fn get(&self, id: &str) -> Option<(&'static str, &Answer)> {
        self.fields
            .iter()
            .zip(&self.answers)
            .find(|(field, _)| field.id == id)
            .map(|(field, answer)| (field.label, answer))
    }

    /// The text of an optional line, text or choice answer.
    fn optional(&self, id: &str) -> Option<String> {
        match self.get(id)?.1 {
            Answer::Line(text) | Answer::Text(text) => Some(text.clone()),
            Answer::Choice(option) => Some((*option).to_owned()),
            Answer::Absent | Answer::List(_) => None,
        }
    }

    /// The text of a required answer. An absent answer is already reported.
    fn required(&self, id: &'static str) -> Result<String, FormError> {
        let label = self.get(id).map_or(id, |(label, _)| label);
        self.optional(id).ok_or(FormError::Missing { label })
    }

    /// The items of a list answer. Empty when absent.
    fn list(&self, id: &str) -> Vec<String> {
        match self.get(id) {
            Some((_, Answer::List(items))) => items.clone(),
            _ => Vec::new(),
        }
    }

    /// The records of `kind` from answers that hold no problem.
    pub(super) fn form(&self, kind: FormKind) -> Result<Form, FormError> {
        let answers = match kind {
            FormKind::Submit => return Ok(Form::Submit(Box::new(self.submit()?))),
            FormKind::Correctness => JudgmentAnswers::Correctness {
                finding: self.required("finding")?,
            },
            FormKind::Supersedes => JudgmentAnswers::Supersedes {
                by: self.required("by")?,
            },
            FormKind::Kind => JudgmentAnswers::Kind {
                value: self.required("value")?,
            },
            FormKind::Topic => JudgmentAnswers::Topic {
                topics: self.list("topics"),
            },
            FormKind::Demotion => JudgmentAnswers::Demotion {
                to: self.required("to")?,
            },
            FormKind::Maintainer => JudgmentAnswers::Maintainer {
                status: self.required("status")?,
            },
            FormKind::FeaturedApproval => JudgmentAnswers::FeaturedApproval,
        };
        Ok(Form::Judgment(JudgmentForm {
            entry: self.required("entry")?,
            evidence: self.list("evidence"),
            answers,
        }))
    }

    fn submit(&self) -> Result<SubmitForm, FormError> {
        Ok(SubmitForm {
            name: self.required("name")?,
            kind: self.required("kind")?,
            repo: self.required("repo")?,
            crates: self.list("crates"),
            topics: self.list("topics"),
            category: self.optional("category"),
            description: self.required("description")?,
            description_source: self.required("description_source")?,
            description_source_url: self.optional("description_source_url"),
            homepage: self.optional("homepage"),
            homebrew: self.optional("homebrew"),
            docker: self.list("docker"),
            repology: self.optional("repology"),
            evidence_of_use: self.list("evidence_of_use"),
            affiliation: self.required("affiliation")?,
        })
    }
}

impl Form {
    /// The form these answers come from.
    pub fn kind(&self) -> FormKind {
        match self {
            Self::Submit(_) => FormKind::Submit,
            Self::Judgment(form) => match form.answers {
                JudgmentAnswers::Correctness { .. } => FormKind::Correctness,
                JudgmentAnswers::Supersedes { .. } => FormKind::Supersedes,
                JudgmentAnswers::Kind { .. } => FormKind::Kind,
                JudgmentAnswers::Topic { .. } => FormKind::Topic,
                JudgmentAnswers::Demotion { .. } => FormKind::Demotion,
                JudgmentAnswers::FeaturedApproval => FormKind::FeaturedApproval,
                JudgmentAnswers::Maintainer { .. } => FormKind::Maintainer,
            },
        }
    }
}
