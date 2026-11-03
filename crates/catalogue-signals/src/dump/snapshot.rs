//! The data kept from one crates.io database dump.

use std::collections::{HashMap, HashSet};

use catalogue_core::deprecation::DeprecationNotice;
use jiff::{Timestamp, civil};
use rustc_hash::FxBuildHasher;

/// A map keyed by a crates.io integer id.
///
/// The ids come from crates.io's own sequences, so the map uses the fast
/// non-keyed Fx hash.
pub type IdMap<V> = HashMap<u32, V, FxBuildHasher>;

/// A set of crates.io integer ids, hashed like [`IdMap`].
pub type IdSet = HashSet<u32, FxBuildHasher>;

/// Everything the reader keeps from one dump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpSnapshot {
    /// The dump's `metadata.json`.
    pub meta: DumpMeta,
    /// Crates by crate id.
    pub crates: IdMap<CrateRow>,
    /// Versions by version id.
    pub versions: IdMap<VersionRow>,
    /// Default version id by crate id.
    pub default_version: IdMap<u32>,
    /// Every row of the `dependencies` table.
    pub edges: Vec<DependencyEdge>,
    /// The requirements of each crate's default version, by depender crate id.
    pub default_requirements: IdMap<Vec<Requirement>>,
    /// Owners by crate id.
    pub owners: IdMap<Vec<Owner>>,
    /// User login by user id.
    pub users: IdMap<String>,
    /// Team login by team id.
    pub teams: IdMap<String>,
    /// Category slugs by crate id.
    pub categories: IdMap<Vec<String>>,
    /// Keywords by crate id.
    pub keywords: IdMap<Vec<String>>,
    /// Crates removed from crates.io.
    pub deleted: Vec<DeletedCrate>,
    /// Daily download totals in the window, by crate id.
    pub downloads: IdMap<DailyDownloads>,
    /// Rows the reader skipped because they reference an unknown id.
    pub warnings: DumpWarnings,
}

/// The fields of the dump's `metadata.json`, plus its directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpMeta {
    /// When crates.io took the dump.
    pub timestamp: Timestamp,
    /// The crates.io commit that produced the dump.
    pub crates_io_commit: String,
    /// The top-level archive directory, such as `2026-11-03-020017`.
    pub directory: String,
}

/// One row of the `crates` table, without the README.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateRow {
    /// The crate name.
    pub name: String,
    /// When the crate was first published.
    pub created_at: Timestamp,
    /// When crates.io last changed the crate row.
    pub updated_at: Timestamp,
    /// The description. An empty value reads as `None`.
    pub description: Option<String>,
    /// The homepage link. An empty value reads as `None`.
    pub homepage: Option<String>,
    /// The repository link. An empty value reads as `None`.
    pub repository: Option<String>,
    /// The deprecation notice found in the description or README.
    pub deprecation: Option<DeprecationNotice>,
}

/// One row of the `versions` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRow {
    /// The crate id.
    pub crate_id: u32,
    /// The version number, such as `1.2.0-rc.1`.
    pub num: String,
    /// When the version was published.
    pub created_at: Timestamp,
    /// Whether the version is yanked.
    pub yanked: bool,
    /// The declared `rust-version`. An empty value reads as `None`.
    pub rust_version: Option<String>,
    /// The declared edition year, such as `2021`. An empty value reads as `None`.
    pub edition: Option<u16>,
    /// The user id that published the version. A NULL value reads as `None`.
    pub published_by: Option<u32>,
}

/// The `dependencies.kind` code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DependencyKind {
    /// Code 0: a `[dependencies]` entry.
    Normal = 0,
    /// Code 1: a `[build-dependencies]` entry.
    Build = 1,
    /// Code 2: a `[dev-dependencies]` entry.
    Dev = 2,
}

/// One row of the `dependencies` table, without the requirement text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DependencyEdge {
    /// The depending version id.
    pub version_id: u32,
    /// The crate id the version depends on.
    pub crate_id: u32,
    /// The dependency kind.
    pub kind: DependencyKind,
    /// Whether the dependency is optional.
    pub optional: bool,
}

/// One dependency of a crate's default version.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Requirement {
    /// The version requirement, such as `^1.0`.
    pub req: String,
    /// The crate id depended on.
    pub crate_id: u32,
    /// The dependency kind.
    pub kind: DependencyKind,
    /// Whether the dependency is optional.
    pub optional: bool,
}

/// The `crate_owners.owner_kind` code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OwnerKind {
    /// Code 0: the id is a `users.id`.
    User = 0,
    /// Code 1: the id is a `teams.id`.
    Team = 1,
}

/// One owner of a crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Owner {
    /// The user or team id.
    pub id: u32,
    /// Which table `id` refers to.
    pub kind: OwnerKind,
}

/// One row of the `deleted_crates` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedCrate {
    /// The deleted crate name.
    pub name: String,
    /// When crates.io deleted the crate.
    pub deleted_at: Timestamp,
}

/// Daily download totals for one crate, summed over its versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyDownloads {
    /// The first day of the window.
    pub start: civil::Date,
    /// Downloads per day. Index 0 is `start`. The length is the window length.
    pub days: Vec<u32>,
}

/// Counts of rows skipped because they reference an unknown id.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DumpWarnings {
    /// `crates_categories` rows whose `category_id` names no category.
    pub unknown_category: u64,
    /// `crates_keywords` rows whose `keyword_id` names no keyword.
    pub unknown_keyword: u64,
    /// In-window `version_downloads` rows whose `version_id` names no version.
    pub unknown_download_version: u64,
}
