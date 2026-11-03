//! Reads a rendered issue form body into the answers of its form.

use super::answer::read;
use super::error::FormError;
use super::kind::FormKind;
use super::markdown::split;
use super::records::{Answers, Form};

/// Reads `body`, the issue body that GitHub rendered from the `kind` form.
///
/// Accepts only the form's own section headings, in template order. Fails
/// with every problem found. One bad field does not hide the others.
pub fn parse_form(kind: FormKind, body: &str) -> Result<Form, Vec<FormError>> {
    let fields = kind.fields();
    let mut errors = Vec::new();
    let raw = split(body, fields, &mut errors);
    let answers = fields
        .iter()
        .zip(&raw)
        .map(|(field, raw)| read(field, raw.as_deref(), &mut errors))
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    Answers::new(fields, answers)
        .form(kind)
        .map_err(|error| vec![error])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::records::JudgmentAnswers;

    #[test]
    fn reads_a_judgment_form() {
        let body = "### Entry slug\n\nalpha\n\n### Status\n\nfinished\n\n\
                    ### Evidence links\n\n_No response_\n";
        let Ok(Form::Judgment(form)) = parse_form(FormKind::Maintainer, body) else {
            panic!("expected a judgment form");
        };
        assert_eq!(form.entry, "alpha");
        assert!(form.evidence.is_empty());
        assert_eq!(
            form.answers,
            JudgmentAnswers::Maintainer {
                status: "finished".to_owned()
            }
        );
    }

    #[test]
    fn collects_every_problem() {
        let body = "### Entry slug\n\n_No response_\n\n### Status\n\nretired\n";
        let errors = parse_form(FormKind::Maintainer, body).expect_err("problems");
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(
            errors.first(),
            Some(&FormError::NoResponse {
                label: "Entry slug"
            })
        );
        assert!(matches!(
            errors.get(1),
            Some(FormError::BadChoice {
                label: "Status",
                ..
            })
        ));
    }
}
