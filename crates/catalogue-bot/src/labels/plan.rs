//! The changes that bring the repo labels in line with the labels file.

use std::collections::{HashMap, HashSet};

use super::file::{LabelSpec, LabelsFile, fold};

/// A label as the repo holds it now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoLabel {
    /// The label name.
    pub name: String,
    /// The color as 6 hex digits, without `#`.
    pub color: String,
    /// The description. Empty when the repo has none.
    pub description: String,
}

/// A repo label whose name case, color or description differs from the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelUpdate {
    /// The label as the repo holds it.
    pub current: RepoLabel,
    /// The label as the file writes it.
    pub wanted: LabelSpec,
}

/// What a sync does. It never deletes a label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncPlan {
    /// Labels in the file and not on the repo, in file order.
    pub create: Vec<LabelSpec>,
    /// Labels on both that differ, in file order.
    pub update: Vec<LabelUpdate>,
    /// Names on the repo and not in the file, in repo order. The sync leaves them in place.
    pub unknown: Vec<String>,
}

/// Plans the sync of `existing` repo labels to `file`.
///
/// Names match without regard to case, as GitHub matches them. A match
/// updates when the name case, the color or the description differs. Colors
/// compare lowercase.
pub fn plan_sync(file: &LabelsFile, existing: &[RepoLabel]) -> SyncPlan {
    let mut plan = SyncPlan::default();
    let mut by_name: HashMap<String, &RepoLabel> = HashMap::with_capacity(existing.len());
    for label in existing {
        by_name.entry(fold(&label.name)).or_insert(label);
    }
    for wanted in file.labels() {
        match by_name.get(&fold(&wanted.name)).copied() {
            None => plan.create.push(wanted.clone()),
            Some(current) if differs(current, wanted) => plan.update.push(LabelUpdate {
                current: current.clone(),
                wanted: wanted.clone(),
            }),
            Some(_) => {}
        }
    }
    let known: HashSet<String> = file
        .labels()
        .iter()
        .map(|label| fold(&label.name))
        .collect();
    plan.unknown = existing
        .iter()
        .filter(|label| !known.contains(&fold(&label.name)))
        .map(|label| label.name.clone())
        .collect();
    plan
}

/// Whether the repo label differs from the file in name case, color or description.
fn differs(current: &RepoLabel, wanted: &LabelSpec) -> bool {
    current.name != wanted.name
        || current.color.to_ascii_lowercase() != wanted.color
        || current.description != wanted.description
}

impl SyncPlan {
    /// Whether the plan changes nothing.
    pub fn is_empty(&self) -> bool {
        self.create.is_empty() && self.update.is_empty()
    }

    /// One line per change, then one per unknown label.
    pub fn lines(&self) -> Vec<String> {
        let creates = self.create.iter().map(create_line);
        let updates = self.update.iter().map(update_line);
        let unknown = self
            .unknown
            .iter()
            .map(|name| format!("unknown {name}: not in the labels file, left in place"));
        creates.chain(updates).chain(unknown).collect()
    }
}

/// The line for a label to create.
fn create_line(label: &LabelSpec) -> String {
    format!(
        "create {}: color {}, description {:?}",
        label.name, label.color, label.description
    )
}

/// The line for a label to update, naming each field that changes.
fn update_line(update: &LabelUpdate) -> String {
    let LabelUpdate { current, wanted } = update;
    let mut changes = Vec::new();
    if current.name != wanted.name {
        changes.push(format!("name -> {}", wanted.name));
    }
    let color = current.color.to_ascii_lowercase();
    if color != wanted.color {
        changes.push(format!("color {color} -> {}", wanted.color));
    }
    if current.description != wanted.description {
        changes.push(format!(
            "description {:?} -> {:?}",
            current.description, wanted.description
        ));
    }
    format!("update {}: {}", current.name, changes.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::parse_labels;

    const FILE: &str = r#"
[[label]]
name = "submit"
color = "0e8a16"
description = "Propose a new project"

[[label]]
name = "judgment:kind"
color = "1d76db"
description = "Set an entry as a product or a demo"

[[label]]
name = "needs-fix"
color = "e11d21"
description = "The bot check found errors to fix"

[[label]]
name = "approved"
color = "2ea44f"
description = "Approved"
"#;

    fn repo(name: &str, color: &str, description: &str) -> RepoLabel {
        RepoLabel {
            name: name.to_owned(),
            color: color.to_owned(),
            description: description.to_owned(),
        }
    }

    #[test]
    fn the_plan_creates_updates_and_lists_unknown_labels() {
        let file = parse_labels(FILE).expect("parses");
        let existing = [
            repo("bug", "d73a4a", "Something is broken"),
            repo("Submit", "0E8A16", "Propose a new project"),
            repo("judgment:kind", "1D76DB", "old text"),
            repo("approved", "2EA44F", "Approved"),
        ];
        let plan = plan_sync(&file, &existing);
        let created: Vec<&str> = plan.create.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(created, ["needs-fix"]);
        let updated: Vec<&str> = plan
            .update
            .iter()
            .map(|u| u.current.name.as_str())
            .collect();
        assert_eq!(updated, ["Submit", "judgment:kind"]);
        assert_eq!(plan.unknown, ["bug"]);
        assert!(!plan.is_empty());
        assert_eq!(
            plan.lines(),
            [
                "create needs-fix: color e11d21, description \"The bot check found errors to fix\"",
                "update Submit: name -> submit",
                "update judgment:kind: description \"old text\" -> \"Set an entry as a product or a demo\"",
                "unknown bug: not in the labels file, left in place",
            ]
        );
    }

    #[test]
    fn a_repo_in_line_with_the_file_needs_no_change() {
        let file = parse_labels(FILE).expect("parses");
        let existing: Vec<RepoLabel> = file
            .labels()
            .iter()
            .map(|l| repo(&l.name, &l.color.to_ascii_uppercase(), &l.description))
            .collect();
        let plan = plan_sync(&file, &existing);
        assert!(plan.is_empty());
        assert!(plan.lines().is_empty());
    }

    #[test]
    fn a_color_change_is_named() {
        let file = parse_labels(FILE).expect("parses");
        let existing = [repo("approved", "000000", "Approved")];
        let plan = plan_sync(&file, &existing);
        assert_eq!(
            update_line(&plan.update[0]),
            "update approved: color 000000 -> 2ea44f"
        );
    }
}
