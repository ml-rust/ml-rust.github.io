//! The display names of tiers, kinds and statuses.

use catalogue_core::{EntryKind, MaintainerStatus, ProjectKind, Tier};

/// The display name of `tier`.
pub(crate) fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Featured => "Featured",
        Tier::Listed => "Listed",
        Tier::New => "New",
        Tier::Archived => "Archived",
    }
}

/// The display name of `kind`.
pub(crate) fn kind_name(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Library => "Library",
        EntryKind::App => "App",
    }
}

/// The display name of the project kind `kind`.
pub(crate) fn project_kind_name(kind: ProjectKind) -> &'static str {
    match kind {
        ProjectKind::Product => "Product",
        ProjectKind::Demo => "Demo",
    }
}

/// The display name of the maintainer status `status`.
pub(crate) fn status_name(status: MaintainerStatus) -> &'static str {
    match status {
        MaintainerStatus::Maintained => "Maintained",
        MaintainerStatus::Finished => "Finished",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tier_has_its_name() {
        let names: Vec<&str> = Tier::ALL.into_iter().map(tier_name).collect();
        assert_eq!(names, ["Featured", "Listed", "New", "Archived"]);
    }
}
