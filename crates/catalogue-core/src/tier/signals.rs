use crate::date::Date;
use crate::entry::EntryKind;

/// Raw signals for one entry on one check.
///
/// `None` means the signal is unknown. A rule never reads an unknown signal as zero or false.
/// The three text fields `rustsec_unmaintained`, `deprecation_notice` and
/// `deprecated_requirement` hold the finding when one exists and `None` when none exists.
#[derive(Debug, Clone, PartialEq)]
pub struct Signals {
    /// Library or app. Picks the adoption gates.
    pub kind: EntryKind,
    /// Date of the first release.
    pub first_release: Option<Date>,
    /// The latest version number, such as `1.4.2`.
    pub latest_version: Option<String>,
    /// Number of releases.
    pub release_count: Option<u32>,
    /// Date of the latest release.
    pub last_release: Option<Date>,
    /// Whether every version is yanked.
    pub all_yanked: Option<bool>,
    /// Whether the crate is deleted from crates.io.
    pub crate_deleted: Option<bool>,
    /// Whether every version is a prerelease or `0.0.x`.
    pub all_prerelease: Option<bool>,
    /// Active external users.
    pub active_users: Option<f64>,
    /// New adopters over the last 12 months.
    pub new_adopters: Option<f64>,
    /// The highest active user count on record.
    pub peak_users: Option<f64>,
    /// Active users as a share of the peak, in `[0, 1]`.
    pub former_glory: Option<f64>,
    /// Cleaned downloads per month.
    pub downloads_per_month: Option<u64>,
    /// Id of a RustSec `unmaintained` advisory, when one exists.
    pub rustsec_unmaintained: Option<String>,
    /// Ids of vulnerability advisories still unpatched after the grace window.
    pub unpatched_vulnerabilities: Option<Vec<String>>,
    /// Deprecation text from the description, the README or a badge, when one exists.
    pub deprecation_notice: Option<String>,
    /// A deprecated requirement in the latest release, such as `tokio 0.1`, when one exists.
    pub deprecated_requirement: Option<String>,
    /// Whether the repository is archived.
    pub repo_archived: Option<bool>,
    /// Whether the repository no longer exists.
    pub repo_gone: Option<bool>,
    /// Date of the latest commit by a person, not a bot.
    pub last_human_commit: Option<Date>,
    /// Age in days of the oldest outside issue without a maintainer reply.
    pub oldest_unanswered_days: Option<u32>,
    /// Maintainer replies on issues over the reply window.
    pub maintainer_replies_365: Option<u32>,
    /// Whether published docs exist.
    pub docs_present: Option<bool>,
    /// Whether `.cargo_vcs_info.json` confirms the repository link.
    pub repo_link_verified: Option<bool>,
    /// Binary asset downloads over the last three releases. Apps only.
    pub binary_downloads_last3: Option<u64>,
    /// Binary asset downloads per release. Apps only.
    pub binary_downloads_per_release: Option<u64>,
    /// Homebrew installs on request over 365 days. Apps only.
    pub homebrew_on_request_365d: Option<u64>,
    /// Repology repository families that package the app. Apps only.
    pub repology_families: Option<u32>,
    /// Docker pull growth over 90 days. Apps only.
    pub docker_pull_growth_90d: Option<u64>,
}

impl Signals {
    /// Signals of an entry of `kind` with every value unknown.
    pub fn unknown(kind: EntryKind) -> Self {
        Self {
            kind,
            first_release: None,
            latest_version: None,
            release_count: None,
            last_release: None,
            all_yanked: None,
            crate_deleted: None,
            all_prerelease: None,
            active_users: None,
            new_adopters: None,
            peak_users: None,
            former_glory: None,
            downloads_per_month: None,
            rustsec_unmaintained: None,
            unpatched_vulnerabilities: None,
            deprecation_notice: None,
            deprecated_requirement: None,
            repo_archived: None,
            repo_gone: None,
            last_human_commit: None,
            oldest_unanswered_days: None,
            maintainer_replies_365: None,
            docs_present: None,
            repo_link_verified: None,
            binary_downloads_last3: None,
            binary_downloads_per_release: None,
            homebrew_on_request_365d: None,
            repology_families: None,
            docker_pull_growth_90d: None,
        }
    }

    /// Whether the entry is a library.
    pub fn is_library(&self) -> bool {
        self.kind == EntryKind::Library
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_sets_every_signal_to_none() {
        let signals = Signals::unknown(EntryKind::App);
        assert_eq!(signals.kind, EntryKind::App);
        assert!(!signals.is_library());
        assert!(signals.first_release.is_none());
        assert!(signals.unpatched_vulnerabilities.is_none());
        assert!(signals.docker_pull_growth_90d.is_none());
        assert!(Signals::unknown(EntryKind::Library).is_library());
    }
}
