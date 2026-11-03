//! The dependent counts of one crate.

/// The ecosyste.ms dependent counts of a crates.io package.
///
/// A count the API omits or sets to `null` is `0` here, and its API field name
/// appears in [`fields_missing`](Self::fields_missing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateDependents {
    /// Packages in any registry that depend on the crate
    /// (`dependent_packages_count`).
    pub dependent_packages: u32,
    /// Repos that depend on the crate (`dependent_repos_count`).
    pub dependent_repos: u32,
    /// Docker images that include the crate (`docker_dependents_count`).
    pub docker_dependents: u32,
    /// Downloads of those Docker images (`docker_downloads_count`).
    pub docker_downloads: u64,
    /// The API field names that were absent or `null`, in a fixed order.
    pub fields_missing: Vec<&'static str>,
}
