//! One site build: refusal checks, then every page and asset.

use askama::Template;

use crate::asset::HashedAsset;
use crate::error::SiteError;
use crate::inputs::SiteInputs;
use crate::machine::{
    catalogue_file, compare_files, data_readme_file, feed_file, llms_file, robots_file,
    sitemap_file,
};
use crate::pages::{
    CategoryPage, ChangesPage, ComparePage, FeaturedPage, HomePage, Layout, MlRustPage, NavItem,
    NotFoundPage, PageMeta, ProjectPage, RulesPage, SearchPage, SubmitPage, TopicPage, TopicsPage,
    category_file, project_file, topic_file,
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
    let search_script = HashedAsset::search_script();
    let list_script = HashedAsset::list_script();
    let compare_script = HashedAsset::compare_script();
    let favicon = HashedAsset::favicon();
    let layout = |current, meta| Layout::new(stylesheet.href(), favicon.href(), current, meta);
    let explore = Some(NavItem::Explore);
    let catalogue = inputs.catalogue;
    let report = inputs.report;
    // A list page holds the list script only when it holds something for the script to work on.
    let featured_page = {
        let mut page = FeaturedPage::new(layout(explore, PageMeta::featured()), inputs);
        if page.uses_list_script() {
            page.layout.scripts.push(list_script.href());
        }
        page
    };
    let ml_rust_page = {
        let mut page = MlRustPage::new(layout(explore, PageMeta::ml_rust()), inputs);
        if page.uses_list_script() {
            page.layout.scripts.push(list_script.href());
        }
        page
    };
    let mut files = vec![
        catalogue_file(inputs)?,
        data_readme_file(),
        feed_file(inputs)?,
        llms_file(catalogue),
        robots_file(),
        stylesheet.file(),
        favicon.file(),
        search_script.file(),
        list_script.file(),
        compare_script.file(),
        render(
            "index.html".to_owned(),
            &HomePage::new(layout(explore, PageMeta::home()), inputs),
        )?,
        render("featured/index.html".to_owned(), &featured_page)?,
        render("ml-rust/index.html".to_owned(), &ml_rust_page)?,
        render(
            "topics/index.html".to_owned(),
            &TopicsPage::new(layout(explore, PageMeta::topics()), catalogue, report),
        )?,
        render(
            "changes/index.html".to_owned(),
            &ChangesPage::new(layout(Some(NavItem::Changes), PageMeta::changes()), inputs),
        )?,
        render(
            "rules/index.html".to_owned(),
            &RulesPage::new(
                layout(Some(NavItem::Rules), PageMeta::rules()),
                catalogue.thresholds(),
            ),
        )?,
        render(
            "submit/index.html".to_owned(),
            &SubmitPage::new(
                layout(Some(NavItem::Submit), PageMeta::submit()),
                catalogue.topics(),
                catalogue.categories(),
            ),
        )?,
        render(
            "search/index.html".to_owned(),
            &SearchPage::new(
                // The list script comes first, so it hears every row the search script adds.
                layout(None, PageMeta::search())
                    .with_script(list_script.href())
                    .with_script(search_script.href()),
                catalogue,
                report,
            ),
        )?,
        render(
            "compare/index.html".to_owned(),
            &ComparePage::new(
                // The list script comes first, so it hears when the table is ready.
                layout(None, PageMeta::compare())
                    .with_script(list_script.href())
                    .with_script(compare_script.href()),
            ),
        )?,
        render(
            "404.html".to_owned(),
            &NotFoundPage::new(layout(None, PageMeta::not_found()), catalogue, report),
        )?,
    ];
    for topic in catalogue.topics() {
        let mut page = TopicPage::new(layout(explore, PageMeta::topic(topic)), inputs, topic);
        if page.uses_list_script() {
            page.layout.scripts.push(list_script.href());
        }
        files.push(render(topic_file(&topic.id), &page)?);
    }
    for category in catalogue.categories() {
        let meta = PageMeta::category(category);
        let mut page = CategoryPage::new(layout(explore, meta), inputs, category);
        if page.uses_list_script() {
            page.layout.scripts.push(list_script.href());
        }
        files.push(render(category_file(&category.id), &page)?);
    }
    for slug in catalogue.entries().keys() {
        if let Some(effective) = catalogue.effective(slug) {
            let entry = effective.entry;
            let meta = PageMeta::project(slug, &entry.name, &entry.description.text);
            let mut page = ProjectPage::new(layout(None, meta), inputs, &effective);
            if page.uses_list_script() {
                page.layout.scripts.push(list_script.href());
            }
            files.push(render(project_file(slug), &page)?);
        }
    }
    files.extend(compare_files(inputs)?);
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
