//! `/categories/<id>/`: one category with its blurb and entries.

use askama::Template;
use catalogue_core::Category;

use super::layout::Layout;
use super::shared::{EntryGroup, ListHead, group_entries};
use crate::inputs::SiteInputs;

/// The page of one category.
#[derive(Debug, Template)]
#[template(path = "category.html")]
pub(crate) struct CategoryPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// The category id. It picks the icon.
    pub id: String,
    /// The category name.
    pub name: &'a str,
    /// The sentence that defines the category.
    pub blurb: &'a str,
    /// The entries, strongest tier first, each row showing how to get the project.
    pub groups: Vec<EntryGroup<'a>>,
    /// The parts above the rows.
    pub head: ListHead<'a>,
}

impl<'a> CategoryPage<'a> {
    /// The page of `category` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>, category: &'a Category) -> Self {
        let slugs = inputs.catalogue.entries_in_category(&category.id);
        let groups = group_entries(inputs, &slugs);
        Self {
            layout,
            id: category.id.as_str().to_owned(),
            name: &category.name,
            blurb: &category.blurb,
            head: ListHead::new(inputs, slugs.len(), &groups),
            groups,
        }
    }
}
