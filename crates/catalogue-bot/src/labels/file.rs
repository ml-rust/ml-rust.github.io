//! `.github/labels.toml`: the labels the bot keeps on the repo.

use std::collections::HashSet;
use std::fmt;

use serde::Deserialize;
use thiserror::Error;

/// The longest label description GitHub accepts, in characters.
pub const DESCRIPTION_MAX: usize = 100;

/// One label the repo must carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelSpec {
    /// The label name, exact case.
    pub name: String,
    /// The color as 6 lowercase hex digits, without `#`.
    pub color: String,
    /// The description, at most [`DESCRIPTION_MAX`] characters.
    pub description: String,
}

/// The parsed labels file, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelsFile {
    labels: Vec<LabelSpec>,
}

impl LabelsFile {
    /// Every label, in file order.
    pub fn labels(&self) -> &[LabelSpec] {
        &self.labels
    }

    /// The label named `name`, matched without regard to case as GitHub does.
    pub fn get(&self, name: &str) -> Option<&LabelSpec> {
        let key = fold(name);
        self.labels.iter().find(|label| fold(&label.name) == key)
    }
}

/// One rule a label in the file breaks.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LabelProblem {
    /// The name is empty or has spaces at either end.
    #[error(
        "label {position}: name `{name}` is empty or has spaces at either end. Write the name without them"
    )]
    Name {
        /// The 1-based position of the label in the file.
        position: usize,
        /// The name as written.
        name: String,
    },
    /// The color is not 6 hex digits.
    #[error("label `{name}`: color `{color}` is not 6 hex digits. Write it as rrggbb, without `#`")]
    Color {
        /// The label name.
        name: String,
        /// The color as written.
        color: String,
    },
    /// The description is longer than GitHub allows.
    #[error(
        "label `{name}`: the description has {length} characters. GitHub allows {DESCRIPTION_MAX}. Shorten it"
    )]
    Description {
        /// The label name.
        name: String,
        /// The description length in characters.
        length: usize,
    },
    /// Two labels share a name when case is ignored.
    #[error(
        "label `{name}` appears more than once, ignoring case. GitHub keeps one label per name. Remove the copy"
    )]
    Duplicate {
        /// The repeated name, as the later label writes it.
        name: String,
    },
}

/// A labels file that cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LabelsError {
    /// The text is not TOML in the labels file shape.
    #[error("the labels file is not valid: {0}. Fix it and rerun")]
    Toml(String),
    /// Labels break rules. Every problem is listed.
    #[error("{}", Problems(.0))]
    Invalid(Vec<LabelProblem>),
}

/// Formats problems one per line.
struct Problems<'a>(&'a [LabelProblem]);

impl fmt::Display for Problems<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, problem) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{problem}")?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    label: Vec<RawLabel>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLabel {
    name: String,
    color: String,
    description: String,
}

/// Parses the text of a labels file: `[[label]]` tables with `name`, `color` and `description`.
///
/// Colors come back lowercase.
///
/// # Errors
///
/// Returns [`LabelsError::Toml`] for text in another shape and
/// [`LabelsError::Invalid`] with every broken rule.
pub fn parse_labels(text: &str) -> Result<LabelsFile, LabelsError> {
    let raw: RawFile = toml::from_str(text).map_err(|error| {
        let line = error
            .span()
            .and_then(|span| text.get(..span.start))
            .map(|before| before.matches('\n').count() + 1);
        LabelsError::Toml(match line {
            Some(line) => format!("line {line}: {}", error.message().trim_end()),
            None => error.message().trim_end().to_owned(),
        })
    })?;
    let mut problems = Vec::new();
    let mut seen = HashSet::new();
    let mut labels = Vec::with_capacity(raw.label.len());
    for (index, label) in raw.label.into_iter().enumerate() {
        if label.name.is_empty() || label.name.trim() != label.name {
            problems.push(LabelProblem::Name {
                position: index + 1,
                name: label.name.clone(),
            });
        }
        if label.color.len() != 6 || !label.color.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            problems.push(LabelProblem::Color {
                name: label.name.clone(),
                color: label.color.clone(),
            });
        }
        let length = label.description.chars().count();
        if length > DESCRIPTION_MAX {
            problems.push(LabelProblem::Description {
                name: label.name.clone(),
                length,
            });
        }
        if !seen.insert(fold(&label.name)) {
            problems.push(LabelProblem::Duplicate {
                name: label.name.clone(),
            });
        }
        labels.push(LabelSpec {
            color: label.color.to_ascii_lowercase(),
            name: label.name,
            description: label.description,
        });
    }
    if problems.is_empty() {
        Ok(LabelsFile { labels })
    } else {
        Err(LabelsError::Invalid(problems))
    }
}

/// The form GitHub compares label names in.
pub(super) fn fold(name: &str) -> String {
    name.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
[[label]]
name = "submit"
color = "0E8A16"
description = "Propose a new project"

[[label]]
name = "judgment:kind"
color = "1d76db"
description = ""
"#;

    #[test]
    fn a_good_file_parses_in_order_with_lowercase_colors() {
        let file = parse_labels(GOOD).expect("parses");
        let names: Vec<&str> = file.labels().iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["submit", "judgment:kind"]);
        assert_eq!(file.labels()[0].color, "0e8a16");
        assert_eq!(
            file.get("JUDGMENT:Kind").map(|l| l.color.as_str()),
            Some("1d76db")
        );
        assert!(file.get("bug").is_none());
    }

    #[test]
    fn every_broken_rule_is_reported() {
        let long = "x".repeat(DESCRIPTION_MAX + 1);
        let text = format!(
            r##"
[[label]]
name = " padded"
color = "123456"
description = "a"

[[label]]
name = "red"
color = "#ff0000"
description = "{long}"

[[label]]
name = "Red"
color = "ff000g"
description = "b"
"##
        );
        let Err(LabelsError::Invalid(problems)) = parse_labels(&text) else {
            panic!("expected problems");
        };
        assert_eq!(
            problems,
            [
                LabelProblem::Name {
                    position: 1,
                    name: " padded".to_owned()
                },
                LabelProblem::Color {
                    name: "red".to_owned(),
                    color: "#ff0000".to_owned()
                },
                LabelProblem::Description {
                    name: "red".to_owned(),
                    length: DESCRIPTION_MAX + 1
                },
                LabelProblem::Color {
                    name: "Red".to_owned(),
                    color: "ff000g".to_owned()
                },
                LabelProblem::Duplicate {
                    name: "Red".to_owned()
                },
            ]
        );
        let message = LabelsError::Invalid(problems).to_string();
        assert_eq!(message.lines().count(), 5, "{message}");
    }

    #[test]
    fn a_description_of_exactly_the_limit_passes() {
        let text = format!(
            "[[label]]\nname = \"a\"\ncolor = \"000000\"\ndescription = \"{}\"\n",
            "é".repeat(DESCRIPTION_MAX)
        );
        assert!(parse_labels(&text).is_ok());
    }

    #[test]
    fn another_shape_is_a_toml_error() {
        for text in [
            "label = 1",
            "[[label]]\nname = \"a\"\ncolor = \"000000\"\n",
            "[[label]]\nname = \"a\"\ncolor = \"000000\"\ndescription = \"\"\nextra = 1\n",
            "- name: a",
        ] {
            assert!(
                matches!(parse_labels(text), Err(LabelsError::Toml(_))),
                "{text}"
            );
        }
    }
}
