//! The issue forms in `.github/ISSUE_TEMPLATE/`, as the submit page lists them.

use catalogue_core::ISSUE_URL_PREFIX;

use crate::href::SafeHref;

/// What a person wants to do with a form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormGroup {
    /// Add a project to the catalogue.
    Propose,
    /// Report something about a project in the catalogue.
    Report,
    /// Approve a tier change or state a project's status.
    Approve,
}

/// One issue form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IssueForm {
    /// The template file name in `.github/ISSUE_TEMPLATE/`.
    pub file: &'static str,
    /// The form's `name` line.
    pub name: &'static str,
    /// The form's `description` line.
    pub description: &'static str,
    /// What the form is for.
    pub group: FormGroup,
}

impl IssueForm {
    /// The link that opens a new issue with this form.
    pub fn new_issue_href(&self) -> SafeHref {
        SafeHref::new(&format!("{ISSUE_URL_PREFIX}new?template={}", self.file))
    }
}

/// The form that reports a wrong fact on an entry.
pub const CORRECTNESS_FORM: IssueForm = IssueForm {
    file: "correctness.yml",
    name: "Correctness report",
    description: "Report a wrong fact on an entry.",
    group: FormGroup::Report,
};

/// Every issue form, in the order the submit page lists them.
///
/// A test checks this table against the files in `.github/ISSUE_TEMPLATE/`.
pub const ISSUE_FORMS: [IssueForm; 8] = [
    IssueForm {
        file: "submit.yml",
        name: "Submit a project",
        description: "Propose a new library or app for the catalogue.",
        group: FormGroup::Propose,
    },
    CORRECTNESS_FORM,
    IssueForm {
        file: "supersedes.yml",
        name: "Supersedes or fork",
        description: "Report that one entry replaces another.",
        group: FormGroup::Report,
    },
    IssueForm {
        file: "kind.yml",
        name: "Product or demo",
        description: "Set whether an entry is a product or a demo.",
        group: FormGroup::Report,
    },
    IssueForm {
        file: "topic.yml",
        name: "Topic change",
        description: "Change the topics of an entry.",
        group: FormGroup::Report,
    },
    IssueForm {
        file: "demotion.yml",
        name: "Featured demotion",
        description: "Approve moving a Featured entry to a lower tier.",
        group: FormGroup::Approve,
    },
    IssueForm {
        file: "featured-approval.yml",
        name: "Featured approval",
        description: "Approve Featured for an org-affiliated entry.",
        group: FormGroup::Approve,
    },
    IssueForm {
        file: "maintainer.yml",
        name: "Maintainer statement",
        description: "State whether a project is maintained or finished.",
        group: FormGroup::Approve,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_form_opens_its_template_in_this_repo() {
        assert_eq!(
            ISSUE_FORMS[0].new_issue_href().as_str(),
            "https://github.com/ml-rust/ml-rust.github.io/issues/new?template=submit.yml"
        );
        for form in ISSUE_FORMS {
            let href = form.new_issue_href();
            assert!(!href.is_refused(), "{}", form.file);
            assert!(href.as_str().ends_with(form.file), "{href}");
        }
    }

    #[test]
    fn file_names_are_unique() {
        let mut files: Vec<&str> = ISSUE_FORMS.iter().map(|form| form.file).collect();
        files.sort_unstable();
        files.dedup();
        assert_eq!(files.len(), ISSUE_FORMS.len());
    }
}
