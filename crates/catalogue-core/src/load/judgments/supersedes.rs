use std::collections::BTreeMap;

use super::super::diagnostics::Diagnostics;
use super::super::graph::reaches;
use crate::ids::Slug;
use crate::judgment::{Judgment, JudgmentKind};

/// One `supersedes` judgment: `entry` is replaced by `by`.
pub(super) struct Link {
    pub path: String,
    pub entry: Slug,
    pub by: Slug,
}

impl Link {
    /// The link a `supersedes` judgment makes, or `None` for any other judgment.
    pub(super) fn of(path: &str, judgment: &Judgment) -> Option<Self> {
        Self::of_kind(path, &judgment.entry, &judgment.judgment)
    }

    /// The link a `kind` judgment on `entry` makes, or `None` unless it is `supersedes`.
    pub(super) fn of_kind(path: &str, entry: &Slug, kind: &JudgmentKind) -> Option<Self> {
        match kind {
            JudgmentKind::Supersedes { by } => Some(Self {
                path: path.to_owned(),
                entry: entry.clone(),
                by: by.clone(),
            }),
            _ => None,
        }
    }
}

/// Reports every link that closes a loop of replacements.
pub(super) fn check(links: &[Link], diags: &mut Diagnostics) {
    let mut edges: BTreeMap<Slug, Vec<Slug>> = BTreeMap::new();
    for link in links {
        edges
            .entry(link.entry.clone())
            .or_default()
            .push(link.by.clone());
    }
    for link in links {
        if link.entry != link.by && reaches(&edges, &link.by, &link.entry) {
            diags.push(
                &link.path,
                format!(
                    "supersedes link from `{}` to `{}` forms a cycle",
                    link.entry, link.by
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{errors, put, supersedes_judgment as supersedes, valid};

    #[test]
    fn a_chain_passes() {
        let mut src = valid();
        put(
            &mut src,
            "judgments/7.toml",
            &supersedes(7, "alpha", "beta"),
        );
        assert!(errors(src).is_empty());
    }

    #[test]
    fn a_loop_is_reported_on_each_link() {
        let mut src = valid();
        put(
            &mut src,
            "judgments/7.toml",
            &supersedes(7, "alpha", "beta"),
        );
        put(
            &mut src,
            "judgments/8.toml",
            &supersedes(8, "beta", "alpha"),
        );
        let found = errors(src);
        assert_eq!(
            found.iter().filter(|m| m.contains("forms a cycle")).count(),
            2,
            "{found:?}"
        );
    }
}
