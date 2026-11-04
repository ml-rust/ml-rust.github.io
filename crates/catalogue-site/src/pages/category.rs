//! `/categories/<id>/`: one category with its blurb and entries.

use askama::Template;
use catalogue_core::{Catalogue, Category, TierReport};

use super::layout::Layout;
use super::shared::{EntryGroup, group_install_rows};

/// The page of one category.
#[derive(Debug, Template)]
#[template(path = "category.html")]
pub(crate) struct CategoryPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// The category name.
    pub name: &'a str,
    /// The sentence that defines the category.
    pub blurb: &'a str,
    /// The entries, strongest tier first, each row leading with how to get the project.
    pub groups: Vec<EntryGroup<'a>>,
    /// Whether a report gives the tiers.
    pub tiered: bool,
}

impl<'a> CategoryPage<'a> {
    /// The page of `category` in `layout`, with tiers from `report` when given.
    pub(crate) fn new(
        layout: Layout,
        catalogue: &'a Catalogue,
        report: Option<&TierReport>,
        category: &'a Category,
    ) -> Self {
        let slugs = catalogue.entries_in_category(&category.id);
        Self {
            layout,
            name: &category.name,
            blurb: &category.blurb,
            groups: group_install_rows(catalogue, report, &slugs),
            tiered: report.is_some(),
        }
    }
}
