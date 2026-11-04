//! The input index and the result of drafting.

use std::collections::{HashMap, HashSet};

use catalogue_core::{
    Catalogue, Categories, Entry, Slug, Topics, canonical_crate_name, normalize_repo_url,
};

/// The data files that map crate metadata to the topics and the category of a draft.
#[derive(Debug, Clone, Copy)]
pub struct Mappings<'a> {
    /// The topic mappings. A draft needs at least one topic.
    pub topics: &'a Topics,
    /// The category mappings. A draft can have no category.
    pub categories: &'a Categories,
}

/// The crate names, slugs and repositories of the entries that already exist.
///
/// Crate names use the form crates.io compares: [`canonical_crate_name`].
/// Repositories use the form [`normalize_repo_url`] gives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExistingIndex {
    crates: HashSet<String>,
    slugs: HashSet<String>,
    repos: HashMap<String, String>,
    parents: Vec<(Slug, String)>,
}

impl ExistingIndex {
    /// Indexes the entries of `catalogue`.
    pub fn from_catalogue(catalogue: &Catalogue) -> Self {
        let mut index = Self::default();
        for (slug, entry) in catalogue.entries() {
            index.slugs.insert(slug.as_str().to_owned());
            if let Some(key) = normalize_repo_url(entry.repo.as_str()) {
                index.repos.insert(key, slug.as_str().to_owned());
            }
            for name in &entry.crates {
                index.crates.insert(name.normalized());
            }
            if entry.part_of.is_none()
                && let Some(primary) = entry.crates.first()
            {
                index.parents.push((slug.clone(), primary.normalized()));
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
            repos: HashMap::new(),
            parents: Vec::new(),
        }
    }

    /// Records that the entry `slug` has the primary crate `name` and no parent of its own.
    #[must_use]
    pub fn with_parent(mut self, slug: Slug, name: &str) -> Self {
        self.parents.push((slug, canonical_crate_name(name)));
        self
    }

    /// The entries that can be a parent: slug and canonical primary crate name.
    pub(super) fn parents(&self) -> &[(Slug, String)] {
        &self.parents
    }

    /// Records that the entry `slug` holds the repository `key`.
    #[must_use]
    pub fn with_repo(mut self, key: &str, slug: &str) -> Self {
        self.repos.insert(key.to_owned(), slug.to_owned());
        self
    }

    /// Whether an existing entry lists crate `name`, in any spelling crates.io treats as equal.
    pub fn lists_crate(&self, name: &str) -> bool {
        self.crates.contains(&canonical_crate_name(name))
    }

    /// Whether an existing entry has `slug`.
    pub fn has_slug(&self, slug: &Slug) -> bool {
        self.slugs.contains(slug.as_str())
    }

    /// The slug of the entry that holds the repository `key`.
    pub fn repo_slug(&self, key: &str) -> Option<&str> {
        self.repos.get(key).map(String::as_str)
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
