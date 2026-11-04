//! `/`: the search, the jobs, the categories, the top Featured projects and one line on changes.

use askama::Template;

use super::layout::Layout;
use super::shared::{
    CategoryCard, ChangeRow, EntryGroup, ExampleSearch, FirstCheckLine, TopicCard, category_cards,
    change_rows, checked_dates, date_text, example_searches, featured_order_text, featured_rows,
    featured_slugs, first_check_line, project_count, topic_cards,
};
use crate::inputs::SiteInputs;

/// How many project families the Featured list of the home page shows.
const FEATURED_FAMILIES: usize = 8;

/// The home page.
#[derive(Debug, Template)]
#[template(path = "index.html")]
pub(crate) struct HomePage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// The run date of the check behind the tiers. `None` without a report.
    pub checked: Option<String>,
    /// The run date as a person reads it. `None` without a report.
    pub checked_text: Option<String>,
    /// The counts of the catalogue, as the start of the sentence below the headline.
    pub counts: String,
    /// The example searches.
    pub examples: Vec<ExampleSearch>,
    /// One tile per top-level topic, in file order.
    pub topics: Vec<TopicCard<'a>>,
    /// One tile per category, in file order.
    pub categories: Vec<CategoryCard<'a>>,
    /// The first Featured project families. Empty without a report.
    pub groups: Vec<EntryGroup<'a>>,
    /// How many entries are Featured.
    pub featured_total: usize,
    /// The sentence that states the order of the Featured list.
    pub order: &'static str,
    /// The latest tier change. `None` without a report or without a change.
    pub latest_change: Option<ChangeRow<'a>>,
    /// The latest tier change date as a person reads it.
    pub latest_change_date: Option<String>,
    /// The line that stands for the first check. `None` without a report.
    pub first_check: Option<FirstCheckLine>,
    /// The first check date as a person reads it.
    pub first_check_date: Option<String>,
}

impl<'a> HomePage<'a> {
    /// The home page of `inputs` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>) -> Self {
        let catalogue = inputs.catalogue;
        let report = inputs.report;
        let (checked, checked_text) = checked_dates(inputs);
        let mut featured = featured_rows(inputs);
        featured.truncate(FEATURED_FAMILIES);
        let featured_total = featured_slugs(inputs).len();
        let groups = if featured.is_empty() {
            Vec::new()
        } else {
            vec![EntryGroup::plain(featured)]
        };
        let projects = project_count(catalogue.entries().len());
        let counts = if report.is_some() {
            format!("{projects}, {featured_total} of them Featured.")
        } else {
            format!("{projects}.")
        };
        let latest_change = report.and_then(|_| {
            change_rows(catalogue, inputs.changes, inputs.first_check, 1)
                .into_iter()
                .next()
        });
        let first_check = report.and_then(|_| first_check_line(inputs.changes, inputs.first_check));
        Self {
            layout,
            checked,
            checked_text,
            counts,
            examples: example_searches(),
            topics: topic_cards(catalogue, report),
            categories: category_cards(catalogue),
            groups,
            featured_total,
            order: featured_order_text(),
            latest_change_date: latest_change
                .as_ref()
                .map(|change| date_text(&change.date).unwrap_or_else(|| change.date.clone())),
            latest_change,
            first_check_date: first_check
                .as_ref()
                .map(|line| date_text(&line.date).unwrap_or_else(|| line.date.clone())),
            first_check,
        }
    }
}
