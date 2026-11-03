//! One site build: refusal checks, then every page and asset.

use askama::Template;

use crate::asset::Stylesheet;
use crate::error::SiteError;
use crate::inputs::SiteInputs;
use crate::pages::{Layout, NavItem, NotFoundPage, RulesPage, SubmitPage};
use crate::refusal;

/// One file of the built site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFile {
    /// The path below the site root, `/`-separated, with no leading `/`.
    pub path: String,
    /// The file contents.
    pub bytes: Vec<u8>,
}

/// Builds every page and asset of the site, sorted by path.
///
/// The refusal checks run before any page renders. Without a report, no page
/// shows a tier.
///
/// # Errors
///
/// Returns the first refusal from the checks on `inputs`, or
/// [`SiteError::Render`] when a template fails.
pub fn build_site(inputs: &SiteInputs<'_>) -> Result<Vec<OutputFile>, SiteError> {
    refusal::check(inputs)?;
    let stylesheet = Stylesheet::site();
    let layout = |current| Layout::new(stylesheet.href(), current);
    let catalogue = inputs.catalogue;
    let mut files = vec![
        stylesheet.file(),
        render(
            "rules/index.html",
            &RulesPage::new(layout(Some(NavItem::Rules)), catalogue.thresholds()),
        )?,
        render(
            "submit/index.html",
            &SubmitPage::new(layout(Some(NavItem::Submit)), catalogue.topics()),
        )?,
        render("404.html", &NotFoundPage::new(layout(None)))?,
    ];
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// `page` rendered to the file at `path`.
fn render(path: &'static str, page: &impl Template) -> Result<OutputFile, SiteError> {
    let html = page
        .render()
        .map_err(|source| SiteError::Render { path, source })?;
    Ok(OutputFile {
        path: path.to_owned(),
        bytes: html.into_bytes(),
    })
}
