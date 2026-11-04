//! The fields of each issue form, in the order of `.github/ISSUE_TEMPLATE/*.yml`.

/// How a field's answer is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// An `input`: one line of text.
    Line,
    /// A `textarea` read as free text.
    Text,
    /// A `textarea` read as one item per line.
    List,
    /// A `dropdown`: one of the listed options, matched exactly.
    Choice(&'static [&'static str]),
}

/// One field of an issue form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// The `id` in the form template.
    pub id: &'static str,
    /// The `label`, which GitHub renders as the `### <label>` heading.
    pub label: &'static str,
    /// Whether the form requires an answer.
    pub required: bool,
    /// How the answer is read.
    pub shape: Shape,
}

const fn field(id: &'static str, label: &'static str, required: bool, shape: Shape) -> Field {
    Field {
        id,
        label,
        required,
        shape,
    }
}

/// The `affiliation` answer of a reporter who has no tie to the project.
pub const NOT_AFFILIATED: &str = "I am not affiliated with this project.";
/// The `affiliation` answer of a reporter who maintains or works on the project.
pub const AFFILIATED: &str = "I maintain or work on this project.";

/// The label of field `id` in `fields`, or `id` itself when no field has it.
pub(crate) fn label(fields: &'static [Field], id: &'static str) -> &'static str {
    fields
        .iter()
        .find(|field| field.id == id)
        .map_or(id, |field| field.label)
}

const ENTRY: Field = field("entry", "Entry slug", true, Shape::Line);
const EVIDENCE: Field = field("evidence", "Evidence links", true, Shape::List);

/// The `submit` form.
pub const SUBMIT: &[Field] = &[
    field("name", "Project name", true, Shape::Line),
    field("kind", "Kind", true, Shape::Choice(&["library", "app"])),
    field("repo", "Repository URL", true, Shape::Line),
    field("crates", "Crates", false, Shape::List),
    field("topics", "Topics", true, Shape::List),
    field("category", "Category", false, Shape::Line),
    field("description", "Description", true, Shape::Text),
    field(
        "description_source",
        "Description source",
        true,
        Shape::Choice(&["readme", "human"]),
    ),
    field(
        "description_source_url",
        "Description source URL",
        false,
        Shape::Line,
    ),
    field("homepage", "Homepage", false, Shape::Line),
    field("homebrew", "Homebrew formula", false, Shape::Line),
    field("docker", "Docker images", false, Shape::List),
    field("repology", "Repology project", false, Shape::Line),
    field("evidence_of_use", "Evidence of use", false, Shape::List),
    field(
        "affiliation",
        "Affiliation",
        true,
        Shape::Choice(&[NOT_AFFILIATED, AFFILIATED]),
    ),
];

/// The `judgment:correctness` form.
pub const CORRECTNESS: &[Field] = &[
    ENTRY,
    field("finding", "Finding", true, Shape::Text),
    EVIDENCE,
];

/// The `judgment:supersedes` form.
pub const SUPERSEDES: &[Field] = &[
    ENTRY,
    field("by", "Superseded by", true, Shape::Line),
    EVIDENCE,
];

/// The `judgment:kind` form.
pub const KIND: &[Field] = &[
    ENTRY,
    field("value", "Kind", true, Shape::Choice(&["product", "demo"])),
    EVIDENCE,
];

/// The `judgment:category` form.
pub const CATEGORY: &[Field] = &[
    ENTRY,
    field("topics", "Topics", true, Shape::List),
    EVIDENCE,
];

/// The `judgment:demotion` form.
pub const DEMOTION: &[Field] = &[
    ENTRY,
    field(
        "to",
        "Target tier",
        true,
        Shape::Choice(&["listed", "new", "archived"]),
    ),
    EVIDENCE,
];

/// The `judgment:featured-approval` form.
pub const FEATURED_APPROVAL: &[Field] = &[ENTRY, EVIDENCE];

/// The `judgment:maintainer` form. Evidence is optional here.
pub const MAINTAINER: &[Field] = &[
    ENTRY,
    field(
        "status",
        "Status",
        true,
        Shape::Choice(&["maintained", "finished"]),
    ),
    field("evidence", "Evidence links", false, Shape::List),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_looks_up_the_id() {
        assert_eq!(label(SUBMIT, "repo"), "Repository URL");
        assert_eq!(label(SUBMIT, "ghost"), "ghost");
    }

    #[test]
    fn every_form_has_unique_ids_and_labels() {
        for fields in [
            SUBMIT,
            CORRECTNESS,
            SUPERSEDES,
            KIND,
            CATEGORY,
            DEMOTION,
            FEATURED_APPROVAL,
            MAINTAINER,
        ] {
            for (index, a) in fields.iter().enumerate() {
                for b in fields.iter().skip(index + 1) {
                    assert_ne!(a.id, b.id);
                    assert_ne!(a.label, b.label);
                }
            }
        }
    }
}
