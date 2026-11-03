//! The problems found in an issue form body.

use thiserror::Error;

/// Longest excerpt of an answer that a message repeats, in characters.
const EXCERPT_CHARS: usize = 60;

/// One problem in an issue form body. Each names the field it concerns.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FormError {
    /// Text stands above the first section heading.
    #[error("text stands before the first section `{first}`. Put every answer under its heading")]
    TextBeforeFirstSection {
        /// The label of the first field.
        first: &'static str,
    },
    /// A required section heading is absent.
    #[error("section `{label}` is missing. The form requires it")]
    Missing {
        /// The field label.
        label: &'static str,
    },
    /// A required section holds no answer.
    #[error("section `{label}` has no answer. The form requires one")]
    NoResponse {
        /// The field label.
        label: &'static str,
    },
    /// A section heading appears more than once.
    #[error(
        "section `{label}` appears {count} times. Remove every `### {label}` line except the form's own"
    )]
    Duplicate {
        /// The field label.
        label: &'static str,
        /// How often the heading appears.
        count: usize,
    },
    /// A section heading stands after a field that the form puts later.
    #[error("section `{label}` comes after `{after}`. Keep the form order")]
    OutOfOrder {
        /// The field label.
        label: &'static str,
        /// The label of the section above it.
        after: &'static str,
    },
    /// A one-line field holds several lines.
    #[error("section `{label}` holds {lines} lines. The field takes one line")]
    MultiLine {
        /// The field label.
        label: &'static str,
        /// The number of lines found.
        lines: usize,
    },
    /// A dropdown answer is not one of its options.
    #[error(
        "section `{label}` holds `{value}`, which is not an option. Choose one of: {}",
        .options.join(" | ")
    )]
    BadChoice {
        /// The field label.
        label: &'static str,
        /// An excerpt of the answer found.
        value: String,
        /// The options the form offers.
        options: &'static [&'static str],
    },
    /// The issue link for the issue number does not parse.
    #[error("the issue link for issue {number} does not parse")]
    IssueLink {
        /// The issue number.
        number: u64,
    },
    /// An answer has the right shape and breaks a data rule.
    #[error("section `{label}`: {reason}")]
    Invalid {
        /// The field label.
        label: &'static str,
        /// What is wrong.
        reason: String,
    },
}

/// The first line of `value`, cut to a short excerpt that ends with `…` when cut.
pub(crate) fn excerpt(value: &str) -> String {
    let line = value.lines().next().unwrap_or_default();
    let mut out: String = line.chars().take(EXCERPT_CHARS).collect();
    if out.len() < value.len() {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_keeps_a_short_line_whole() {
        assert_eq!(excerpt("library"), "library");
    }

    #[test]
    fn excerpt_cuts_long_and_multiline_values() {
        let long = "x".repeat(80);
        assert_eq!(excerpt(&long), format!("{}…", "x".repeat(60)));
        assert_eq!(excerpt("app\nmore"), "app…");
    }

    #[test]
    fn messages_name_the_field() {
        let error = FormError::BadChoice {
            label: "Kind",
            value: "App".to_owned(),
            options: &["library", "app"],
        };
        assert_eq!(
            error.to_string(),
            "section `Kind` holds `App`, which is not an option. Choose one of: library | app"
        );
    }
}
