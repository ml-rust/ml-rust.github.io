//! One site build: refusal checks, then every page and asset.

use askama::Template;

use crate::asset::HashedAsset;
use crate::error::SiteError;
use crate::inputs::SiteInputs;
use crate::machine::{
    catalogue_file, data_readme_file, feed_file, llms_file, robots_file, sitemap_file,
};
use crate::pages::{
    CategoryPage, ChangesPage, FeaturedPage, HomePage, Layout, NavItem, NotFoundPage, ProjectPage,
    RulesPage, SearchPage, SubmitPage, TopicPage, TopicsPage, category_file, project_file,
    topic_file,
};
use crate::refusal;
use crate::search::{search_files, search_records};

/// One file of the built site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFile {
    /// The path below the site root, `/`-separated, with no leading `/`.
    pub path: String,
    /// The file contents.
    pub bytes: Vec<u8>,
}

/// Builds every page, asset and search index file of the site, sorted by path.
///
/// The search page loads `search/oxidoc_search.js`, the wasm module of the
/// search engine. This build does not emit it: the deploy adds it.
///
/// The refusal checks run before any page renders. Without a report, no page
/// shows a tier and the search index holds no tier.
///
/// # Errors
///
/// Returns the first refusal from the checks on `inputs`. Returns
/// [`SiteError::Render`] when a template fails, [`SiteError::Search`] when the
/// search index fails to encode, and [`SiteError::Encode`] when the data file
/// fails to encode.
pub fn build_site(inputs: &SiteInputs<'_>) -> Result<Vec<OutputFile>, SiteError> {
    refusal::check(inputs)?;
    let stylesheet = HashedAsset::stylesheet();
    let script = HashedAsset::search_script();
    let favicon = HashedAsset::favicon();
    let layout = |current| Layout::new(stylesheet.href(), favicon.href(), current);
    let catalogue = inputs.catalogue;
    let report = inputs.report;
    let mut files = vec![
        catalogue_file(inputs)?,
        data_readme_file(),
        feed_file(inputs)?,
        llms_file(catalogue),
        robots_file(),
        stylesheet.file(),
        favicon.file(),
        script.file(),
        render(
            "index.html".to_owned(),
            &HomePage::new(layout(Some(NavItem::Explore)), inputs),
        )?,
        render(
            "featured/index.html".to_owned(),
            &FeaturedPage::new(layout(Some(NavItem::Explore)), inputs),
        )?,
        render(
            "topics/index.html".to_owned(),
            &TopicsPage::new(layout(Some(NavItem::Explore)), catalogue, report),
        )?,
        render(
            "changes/index.html".to_owned(),
            &ChangesPage::new(layout(Some(NavItem::Changes)), inputs),
        )?,
        render(
            "rules/index.html".to_owned(),
            &RulesPage::new(layout(Some(NavItem::Rules)), catalogue.thresholds()),
        )?,
        render(
            "submit/index.html".to_owned(),
            &SubmitPage::new(
                layout(Some(NavItem::Submit)),
                catalogue.topics(),
                catalogue.categories(),
            ),
        )?,
        render(
            "search/index.html".to_owned(),
            &SearchPage::new(layout(None), script.href(), catalogue, report),
        )?,
        render(
            "404.html".to_owned(),
            &NotFoundPage::new(layout(None), catalogue, report),
        )?,
    ];
    for topic in catalogue.topics() {
        let page = TopicPage::new(layout(Some(NavItem::Explore)), inputs, topic);
        files.push(render(topic_file(&topic.id), &page)?);
    }
    for category in catalogue.categories() {
        let page = CategoryPage::new(layout(Some(NavItem::Explore)), inputs, category);
        files.push(render(category_file(&category.id), &page)?);
    }
    for slug in catalogue.entries().keys() {
        if let Some(effective) = catalogue.effective(slug) {
            let page = ProjectPage::new(layout(None), inputs, &effective);
            files.push(render(project_file(slug), &page)?);
        }
    }
    let sitemap = sitemap_file(&files)?;
    files.push(sitemap);
    files.extend(search_files(&search_records(catalogue, report))?);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// `page` rendered to the file at `path`.
pub(crate) fn render(path: String, page: &impl Template) -> Result<OutputFile, SiteError> {
    match page.render() {
        Ok(html) => Ok(OutputFile {
            path,
            bytes: html.into_bytes(),
        }),
        Err(source) => Err(SiteError::Render { path, source }),
    }
}
