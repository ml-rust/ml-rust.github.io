//! Reads one raw answer by the shape of its field.

use super::error::{FormError, excerpt};
use super::fields::{Field, Shape};

/// What GitHub renders for an optional field left empty.
pub(crate) const NO_RESPONSE: &str = "_No response_";

/// One normalised answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Answer {
    /// No answer, or an answer with a problem that is already reported.
    Absent,
    /// A one-line answer, trimmed.
    Line(String),
    /// A free text answer, trimmed.
    Text(String),
    /// The items of a list answer, at least one.
    List(Vec<String>),
    /// The dropdown option chosen.
    Choice(&'static str),
}

/// Reads `raw`, the trimmed answer of `field`, and pushes any problem to `errors`.
///
/// `None` stands for an absent or rejected section, which is already reported.
pub(super) fn read(field: &Field, raw: Option<&str>, errors: &mut Vec<FormError>) -> Answer {
    let Some(raw) = raw else {
        return Answer::Absent;
    };
    let text = raw.trim();
    if text.is_empty() || text == NO_RESPONSE {
        return no_answer(field, errors);
    }
    match field.shape {
        Shape::Line => {
            let lines = text.lines().count();
            if lines > 1 {
                errors.push(FormError::MultiLine {
                    label: field.label,
                    lines,
                });
                Answer::Absent
            } else {
                Answer::Line(text.to_owned())
            }
        }
        Shape::Text => Answer::Text(text.to_owned()),
        Shape::List => {
            let items: Vec<String> = text.lines().filter_map(list_item).collect();
            if items.is_empty() {
                no_answer(field, errors)
            } else {
                Answer::List(items)
            }
        }
        Shape::Choice(options) => match options.iter().find(|option| **option == text) {
            Some(option) => Answer::Choice(option),
            None => {
                errors.push(FormError::BadChoice {
                    label: field.label,
                    value: excerpt(text),
                    options,
                });
                Answer::Absent
            }
        },
    }
}

/// The empty answer of `field`, reported when the field is required.
fn no_answer(field: &Field, errors: &mut Vec<FormError>) -> Answer {
    if field.required {
        errors.push(FormError::NoResponse { label: field.label });
    }
    Answer::Absent
}

/// One list line without its `- ` or `* ` bullet and surrounding `<...>`. `None` for a blank line.
fn list_item(line: &str) -> Option<String> {
    let line = line.trim_start();
    let line = match line.trim_end() {
        "-" | "*" => "",
        _ => line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .unwrap_or(line)
            .trim(),
    };
    let line = line
        .strip_prefix('<')
        .and_then(|inner| inner.strip_suffix('>'))
        .unwrap_or(line)
        .trim();
    (!line.is_empty()).then(|| line.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn field(required: bool, shape: Shape) -> Field {
        Field {
            id: "x",
            label: "X",
            required,
            shape,
        }
    }

    fn run(field: &Field, raw: &str) -> (Answer, Vec<FormError>) {
        let mut errors = Vec::new();
        let answer = read(field, Some(raw), &mut errors);
        (answer, errors)
    }

    #[test]
    fn no_response_is_empty_and_reported_when_required() {
        let (answer, errors) = run(&field(false, Shape::Line), NO_RESPONSE);
        assert_eq!(answer, Answer::Absent);
        assert!(errors.is_empty());
        let (answer, errors) = run(&field(true, Shape::Text), " _No response_ ");
        assert_eq!(answer, Answer::Absent);
        assert_eq!(errors, [FormError::NoResponse { label: "X" }]);
    }

    #[test]
    fn an_absent_section_adds_no_problem() {
        let mut errors = Vec::new();
        assert_eq!(
            read(&field(true, Shape::Line), None, &mut errors),
            Answer::Absent
        );
        assert!(errors.is_empty());
    }

    #[test]
    fn a_line_takes_one_line() {
        let line = field(true, Shape::Line);
        assert_eq!(run(&line, "  a b ").0, Answer::Line("a b".to_owned()));
        let (answer, errors) = run(&line, "a\nb");
        assert_eq!(answer, Answer::Absent);
        assert_eq!(
            errors,
            [FormError::MultiLine {
                label: "X",
                lines: 2
            }]
        );
    }

    #[test]
    fn list_items_lose_bullets_brackets_and_blank_lines() {
        let list = field(true, Shape::List);
        let (answer, errors) = run(&list, "- a\n\n* <b>\n  <https://x.io/c>  \nd\n-e\n");
        assert!(errors.is_empty());
        let items = ["a", "b", "https://x.io/c", "d", "-e"].map(str::to_owned);
        assert_eq!(answer, Answer::List(items.to_vec()));
    }

    #[test]
    fn a_list_of_blank_items_has_no_answer() {
        let (answer, errors) = run(&field(true, Shape::List), "- \n<>\n");
        assert_eq!(answer, Answer::Absent);
        assert_eq!(errors, [FormError::NoResponse { label: "X" }]);
    }

    #[test]
    fn a_choice_matches_exactly() {
        let choice = field(true, Shape::Choice(&["library", "app"]));
        assert_eq!(run(&choice, "app").0, Answer::Choice("app"));
        for bad in ["App", "apps", "app.", "library app"] {
            let (answer, errors) = run(&choice, bad);
            assert_eq!(answer, Answer::Absent, "{bad}");
            assert!(
                matches!(errors.as_slice(), [FormError::BadChoice { .. }]),
                "{bad}"
            );
        }
    }
}
