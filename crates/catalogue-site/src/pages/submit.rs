//! `/submit/`: the issue forms, what approval means, and the topic slugs.

use askama::Template;
use catalogue_core::Topic;

use super::layout::Layout;
use crate::forms::ISSUE_FORMS;
use crate::href::SafeHref;

/// The submit page.
#[derive(Debug, Template)]
#[template(path = "submit.html")]
pub(crate) struct SubmitPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// One link per issue form.
    pub forms: Vec<FormLink>,
    /// The topics a form can name, in file order.
    pub topics: Vec<TopicRow<'a>>,
}

/// The link to one issue form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormLink {
    /// The form name.
    pub name: &'static str,
    /// The form description.
    pub description: &'static str,
    /// The link that opens a new issue with the form.
    pub href: SafeHref,
}

/// One topic a form can name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopicRow<'a> {
    /// The topic slug.
    pub id: &'a str,
    /// The display name.
    pub name: &'a str,
    /// The broader topic's slug, or empty.
    pub parent: &'a str,
    /// The link to the topic page.
    pub href: SafeHref,
}

impl<'a> SubmitPage<'a> {
    /// The submit page in `layout`, listing `topics`.
    pub(crate) fn new(layout: Layout, topics: &'a [Topic]) -> Self {
        let forms = ISSUE_FORMS
            .iter()
            .map(|form| FormLink {
                name: form.name,
                description: form.description,
                href: form.new_issue_href(),
            })
            .collect();
        let topics = topics
            .iter()
            .map(|topic| TopicRow {
                id: topic.id.as_str(),
                name: &topic.name,
                parent: topic.parent.as_ref().map_or("", |parent| parent.as_str()),
                href: SafeHref::new(&format!("/topics/{}/", topic.id)),
            })
            .collect();
        Self {
            layout,
            forms,
            topics,
        }
    }
}
