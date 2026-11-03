//! The form an issue uses, chosen from its labels.

use std::fmt;

use thiserror::Error;

use super::fields::{
    CATEGORY, CORRECTNESS, DEMOTION, FEATURED_APPROVAL, Field, KIND, MAINTAINER, SUBMIT, SUPERSEDES,
};

/// The prefix of every judgment form label.
const JUDGMENT_PREFIX: &str = "judgment:";

/// One of the issue forms in `.github/ISSUE_TEMPLATE/`.
///
/// The kind comes from the issue label. The issue title is never read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FormKind {
    /// `submit.yml`, label `submit`.
    Submit,
    /// `correctness.yml`, label `judgment:correctness`.
    Correctness,
    /// `supersedes.yml`, label `judgment:supersedes`.
    Supersedes,
    /// `kind.yml`, label `judgment:kind`.
    Kind,
    /// `category.yml`, label `judgment:category`.
    Category,
    /// `demotion.yml`, label `judgment:demotion`.
    Demotion,
    /// `featured-approval.yml`, label `judgment:featured-approval`.
    FeaturedApproval,
    /// `maintainer.yml`, label `judgment:maintainer`.
    Maintainer,
}

/// The labels of an issue name no form, or more than one.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LabelError {
    /// No label is a form label.
    #[error("no label names a form. Add one of: {}", FormKind::label_list())]
    NoForm,
    /// Two or more labels are form labels.
    #[error("labels {} name more than one form. Keep one", .0.join(", "))]
    SeveralForms(Vec<String>),
}

impl FormKind {
    /// Every form, in the order of the table in the rules.
    pub const ALL: [Self; 8] = [
        Self::Submit,
        Self::Correctness,
        Self::Supersedes,
        Self::Kind,
        Self::Category,
        Self::Demotion,
        Self::FeaturedApproval,
        Self::Maintainer,
    ];

    /// The form part of the label and the template file stem: `submit`, `correctness`, ...
    pub fn name(self) -> &'static str {
        match self {
            Self::Submit => "submit",
            Self::Correctness => "correctness",
            Self::Supersedes => "supersedes",
            Self::Kind => "kind",
            Self::Category => "category",
            Self::Demotion => "demotion",
            Self::FeaturedApproval => "featured-approval",
            Self::Maintainer => "maintainer",
        }
    }

    /// The issue label: `submit`, or `judgment:<name>` for a judgment form.
    pub fn label(self) -> String {
        match self {
            Self::Submit => self.name().to_owned(),
            _ => format!("{JUDGMENT_PREFIX}{}", self.name()),
        }
    }

    /// The template file name under `.github/ISSUE_TEMPLATE/`.
    pub fn template(self) -> String {
        format!("{}.yml", self.name())
    }

    /// The fields of the form, in template order.
    pub fn fields(self) -> &'static [Field] {
        match self {
            Self::Submit => SUBMIT,
            Self::Correctness => CORRECTNESS,
            Self::Supersedes => SUPERSEDES,
            Self::Kind => KIND,
            Self::Category => CATEGORY,
            Self::Demotion => DEMOTION,
            Self::FeaturedApproval => FEATURED_APPROVAL,
            Self::Maintainer => MAINTAINER,
        }
    }

    /// Whether the form records a judgment.
    pub fn is_judgment(self) -> bool {
        self != Self::Submit
    }

    /// The form whose label is exactly `label`. Case and spacing must match.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.label() == label)
    }

    /// The one form that `labels` name. Labels that name no form are ignored.
    pub fn from_labels<'a>(labels: impl IntoIterator<Item = &'a str>) -> Result<Self, LabelError> {
        let mut found: Vec<Self> = labels.into_iter().filter_map(Self::from_label).collect();
        found.sort_unstable();
        found.dedup();
        if found.len() > 1 {
            return Err(LabelError::SeveralForms(
                found.into_iter().map(Self::label).collect(),
            ));
        }
        found.first().copied().ok_or(LabelError::NoForm)
    }

    /// Every form label, comma separated.
    pub fn label_list() -> String {
        let labels: Vec<String> = Self::ALL.into_iter().map(Self::label).collect();
        labels.join(", ")
    }
}

impl fmt::Display for FormKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_round_trip() {
        for kind in FormKind::ALL {
            assert_eq!(FormKind::from_label(&kind.label()), Some(kind));
        }
        assert_eq!(FormKind::Submit.label(), "submit");
        assert_eq!(
            FormKind::FeaturedApproval.label(),
            "judgment:featured-approval"
        );
        assert_eq!(FormKind::Demotion.template(), "demotion.yml");
    }

    #[test]
    fn labels_match_exactly() {
        for bad in [
            "Submit",
            " submit",
            "judgment:Kind",
            "judgment: kind",
            "kind",
            "judgment:",
            "approved",
        ] {
            assert_eq!(FormKind::from_label(bad), None, "{bad}");
        }
    }

    #[test]
    fn one_form_label_is_required() {
        assert_eq!(
            FormKind::from_labels(["approved", "judgment:kind"]),
            Ok(FormKind::Kind)
        );
        assert_eq!(
            FormKind::from_labels(["judgment:kind", "judgment:kind"]),
            Ok(FormKind::Kind)
        );
        assert_eq!(FormKind::from_labels(["approved"]), Err(LabelError::NoForm));
        assert_eq!(
            FormKind::from_labels(["submit", "judgment:kind"]),
            Err(LabelError::SeveralForms(vec![
                "submit".to_owned(),
                "judgment:kind".to_owned()
            ]))
        );
    }
}
