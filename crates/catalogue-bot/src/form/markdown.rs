//! Splits a rendered issue form body into one raw answer per field.
//!
//! GitHub renders each field as a `### <label>` heading, a blank line and the
//! answer, in template order. Every field appears once, so a heading line for
//! a field label that appears twice is a forgery inside an answer. Such a
//! field is rejected and its answer is not read. A heading line for any text
//! other than a field label stays part of the answer above it.

use super::error::FormError;
use super::fields::Field;

/// The section of one heading line: its line number and field index.
#[derive(Debug, Clone, Copy)]
struct Heading {
    line: usize,
    field: usize,
}

/// The raw answer of each field, by field index. `None` for an absent or rejected section.
///
/// Pushes every structural problem to `errors`. The answers keep their inner
/// line breaks and lose the blank lines around them.
pub(super) fn split(
    body: &str,
    fields: &[Field],
    errors: &mut Vec<FormError>,
) -> Vec<Option<String>> {
    let text = body.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let headings: Vec<Heading> = lines
        .iter()
        .enumerate()
        .filter_map(|(line, text)| {
            let title = heading_title(text)?;
            let field = fields.iter().position(|f| f.label == title)?;
            Some(Heading { line, field })
        })
        .collect();

    let counts: Vec<usize> = (0..fields.len())
        .map(|index| headings.iter().filter(|h| h.field == index).count())
        .collect();

    let first_line = headings.first().map_or(lines.len(), |h| h.line);
    let preamble = lines.get(..first_line).unwrap_or_default();
    if preamble.iter().any(|line| !line.trim().is_empty())
        && let Some(first) = fields.first()
    {
        errors.push(FormError::TextBeforeFirstSection { first: first.label });
    }

    let mut values: Vec<Option<String>> = vec![None; fields.len()];
    let mut previous: Option<usize> = None;
    for (index, heading) in headings.iter().enumerate() {
        let Some(field) = fields.get(heading.field) else {
            continue;
        };
        if counts.get(heading.field).copied().unwrap_or_default() > 1 {
            continue;
        }
        if let Some(prior) = previous
            && prior >= heading.field
            && let Some(after) = fields.get(prior)
        {
            errors.push(FormError::OutOfOrder {
                label: field.label,
                after: after.label,
            });
        }
        previous = Some(heading.field);
        let end = headings
            .get(index + 1)
            .map_or(lines.len(), |next| next.line);
        let answer = lines.get(heading.line + 1..end).unwrap_or_default();
        if let Some(slot) = values.get_mut(heading.field) {
            *slot = Some(answer.join("\n").trim().to_owned());
        }
    }

    for (field, count) in fields.iter().zip(&counts) {
        match count {
            0 if field.required => errors.push(FormError::Missing { label: field.label }),
            0 | 1 => {}
            _ => errors.push(FormError::Duplicate {
                label: field.label,
                count: *count,
            }),
        }
    }
    values
}

/// The title of a line that CommonMark renders as a level 3 ATX heading.
///
/// Allows up to three spaces of indent and a closing run of `#`.
fn heading_title(line: &str) -> Option<&str> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = line.get(indent..)?.strip_prefix("###")?;
    if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    let title = rest.trim();
    let closed = title.trim_end_matches('#');
    let title = if closed.len() == title.len() {
        title
    } else if closed.is_empty() {
        ""
    } else if closed.ends_with([' ', '\t']) {
        closed.trim_end()
    } else {
        title
    };
    Some(title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::fields::Shape;

    const FIELDS: &[Field] = &[
        Field {
            id: "a",
            label: "Alpha",
            required: true,
            shape: Shape::Line,
        },
        Field {
            id: "b",
            label: "Beta",
            required: false,
            shape: Shape::Text,
        },
        Field {
            id: "c",
            label: "Gamma",
            required: true,
            shape: Shape::List,
        },
    ];

    fn run(body: &str) -> (Vec<Option<String>>, Vec<FormError>) {
        let mut errors = Vec::new();
        let values = split(body, FIELDS, &mut errors);
        (values, errors)
    }

    fn some(text: &str) -> Option<String> {
        Some(text.to_owned())
    }

    #[test]
    fn splits_sections_in_order() {
        let body = "### Alpha\n\none\n\n### Beta\n\ntwo\nlines\n\n### Gamma\n\nx\ny\n";
        let (values, errors) = run(body);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(values, [some("one"), some("two\nlines"), some("x\ny")]);
    }

    #[test]
    fn crlf_reads_as_lf() {
        let (values, errors) = run("### Alpha\r\n\r\none\r\n\r\n### Gamma\r\n\r\nx\r\ny\r\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(values, [some("one"), None, some("x\ny")]);
    }

    #[test]
    fn other_headings_stay_in_the_answer() {
        let (values, errors) = run("### Alpha\n\none\n### Notes\nmore\n\n### Gamma\n\nx\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(values.first(), Some(&some("one\n### Notes\nmore")));
    }

    #[test]
    fn a_forged_heading_rejects_the_field() {
        let body =
            "### Alpha\n\none\n\n### Beta\n\ntext\n### Gamma\n\nforged\n\n### Gamma\n\nreal\n";
        let (values, errors) = run(body);
        assert_eq!(
            errors,
            [FormError::Duplicate {
                label: "Gamma",
                count: 2
            }]
        );
        assert_eq!(values, [some("one"), some("text"), None]);
    }

    #[test]
    fn indented_and_closed_headings_count_as_headings() {
        let body = "### Alpha\n\none\n\n   ### Gamma ##\n\nforged\n\n### Gamma\n\nreal\n";
        let (_, errors) = run(body);
        assert!(errors.contains(&FormError::Duplicate {
            label: "Gamma",
            count: 2
        }));
    }

    #[test]
    fn missing_required_and_out_of_order_are_reported() {
        let (_, errors) = run("### Beta\n\ntwo\n\n### Alpha\n\none\n");
        assert_eq!(
            errors,
            [
                FormError::OutOfOrder {
                    label: "Alpha",
                    after: "Beta"
                },
                FormError::Missing { label: "Gamma" },
            ]
        );
    }

    #[test]
    fn text_before_the_first_heading_is_reported() {
        let (_, errors) = run("hello\n### Alpha\n\none\n\n### Gamma\n\nx\n");
        assert_eq!(
            errors,
            [FormError::TextBeforeFirstSection { first: "Alpha" }]
        );
    }

    #[test]
    fn an_empty_body_misses_every_required_section() {
        let (values, errors) = run("");
        assert_eq!(values, [None, None, None]);
        assert_eq!(
            errors,
            [
                FormError::Missing { label: "Alpha" },
                FormError::Missing { label: "Gamma" },
            ]
        );
    }

    #[test]
    fn heading_titles_follow_commonmark() {
        assert_eq!(heading_title("### Kind"), Some("Kind"));
        assert_eq!(heading_title("###   Kind   "), Some("Kind"));
        assert_eq!(heading_title("### Kind ###"), Some("Kind"));
        assert_eq!(heading_title("### Kind#"), Some("Kind#"));
        assert_eq!(heading_title("   ### Kind"), Some("Kind"));
        assert_eq!(heading_title("    ### Kind"), None);
        assert_eq!(heading_title("###Kind"), None);
        assert_eq!(heading_title("## Kind"), None);
        assert_eq!(heading_title("#### Kind"), None);
        assert_eq!(heading_title("###"), Some(""));
    }
}
