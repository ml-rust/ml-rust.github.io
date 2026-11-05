//! `/ml-rust/`: every entry the ML Rust organization maintains.

use askama::Template;
use catalogue_core::Slug;

use super::layout::Layout;
use super::shared::{EntryGroup, ListHead, group_entries};
use crate::inputs::SiteInputs;

/// The page of every org-affiliated entry.
#[derive(Debug, Template)]
#[template(path = "ml_rust.html")]
pub(crate) struct MlRustPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// The entries, strongest tier first, each row showing how to get the project.
    pub groups: Vec<EntryGroup<'a>>,
    /// The parts above the rows.
    pub head: ListHead<'a>,
}

impl<'a> MlRustPage<'a> {
    /// The page of `inputs` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>) -> Self {
        let slugs: Vec<&Slug> = inputs
            .catalogue
            .entries()
            .iter()
            .filter(|(_, entry)| entry.org_affiliated)
            .map(|(slug, _)| slug)
            .collect();
        let groups = group_entries(inputs, &slugs);
        Self {
            layout,
            head: ListHead::new(inputs, slugs.len(), &groups),
            groups,
        }
    }

    /// Whether the page holds a table for the list script.
    pub(crate) fn uses_list_script(&self) -> bool {
        !self.groups.is_empty()
    }
}
