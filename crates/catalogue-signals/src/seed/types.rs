//! The input index and the result of drafting.

use std::collections::HashSet;

use catalogue_core::{Catalogue, Entry, Slug, canonical_crate_name};

/// The crate names and slugs of the entries that already exist.
///
/// Crate names use the form crates.io compares: [`canonical_crate_name`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExistingIndex {
    crates: HashSet<String>,
    slugs: HashSet<String>,
}

impl ExistingIndex {
    /// Indexes the entries of `catalogue`.
    pub fn from_catalogue(catalogue: &Catalogue) -> Self {
        let mut index = Self::default();
        for (slug, entry) in catalogue.entries() {
            index.slugs.insert(slug.as_str().to_owned());
            for name in &entry.crates {
                index.crates.insert(name.normalized());
            }
        }
        index
    }

    /// Indexes the given crate names and entry slugs.
    pub fn new<'a>(
        crates: impl IntoIterator<Item = &'a str>,
        slugs: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        Self {
            crates: crates.into_iter().map(canonical_crate_name).collect(),
            slugs: slugs.into_iter().map(str::to_owned).collect(),
        }
    }

    /// Whether an existing entry lists crate `name`, in any spelling crates.io treats as equal.
    pub fn lists_crate(&self, name: &str) -> bool {
        self.crates.contains(&canonical_crate_name(name))
    }

    /// Whether an existing entry has `slug`.
    pub fn has_slug(&self, slug: &Slug) -> bool {
        self.slugs.contains(slug.as_str())
    }
}

/// The result of one drafting run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Drafts {
    /// The drafted entries with their slugs, sorted by slug.
    pub entries: Vec<(Slug, Entry)>,
    /// Crates that yield no draft, as `(crate name, reason)`, sorted by crate name.
    ///
    /// A family that yields none appears under its primary crate. A yanked or
    /// deprecated family member appears under its own name.
    pub skipped: Vec<(String, String)>,
}
