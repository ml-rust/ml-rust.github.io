//! The report of an issue whose labels do not name exactly one form.

use super::report::CheckReport;
use super::submission::text_section;
use crate::form::LabelError;

/// The report of an issue whose labels name no form or several forms. It always blocks.
///
/// The text names only form labels, never issue text.
pub fn label_report(error: &LabelError) -> CheckReport {
    CheckReport {
        blocking: true,
        sections: vec![text_section(
            "Form",
            &format!("The issue needs exactly one form label: {error}."),
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn several_form_labels_block() {
        let error = LabelError::SeveralForms(vec!["submit".to_owned(), "judgment:kind".to_owned()]);
        let report = label_report(&error);
        assert!(report.blocking);
        assert!(
            report.markdown().contains(
                "The issue needs exactly one form label: labels submit, judgment:kind name more than one form. Keep one."
            ),
            "{}",
            report.markdown()
        );
        assert!(label_report(&LabelError::NoForm).blocking);
    }
}
