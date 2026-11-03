//! The unyanked depender versions that name a target crate.

use crate::dump::{DependencyEdge, IdMap, IdSet, VersionRow};

/// One unyanked depender version that names a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Use {
    pub(super) depender: u32,
    pub(super) target: u32,
    pub(super) version: u32,
}

/// The edges into `targets` from unyanked versions of another crate.
pub(super) fn target_uses(
    versions: &IdMap<VersionRow>,
    edges: &[DependencyEdge],
    targets: &IdSet,
) -> Vec<Use> {
    let mut uses = Vec::new();
    for edge in edges {
        if !targets.contains(&edge.crate_id) {
            continue;
        }
        let Some(version) = versions.get(&edge.version_id) else {
            continue;
        };
        if version.yanked || version.crate_id == edge.crate_id {
            continue;
        }
        uses.push(Use {
            depender: version.crate_id,
            target: edge.crate_id,
            version: edge.version_id,
        });
    }
    uses
}
