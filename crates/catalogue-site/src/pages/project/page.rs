//! The entry page struct and its template binding.

use askama::Template;
use catalogue_core::EffectiveEntry;

use super::also_in::{AlsoInRow, also_in};
use super::glance::Glance;
use super::header::Header;
use super::judgments::{CorrectionRow, JudgmentRow};
use super::links::{DescriptionView, LinkRow, link_rows};
use super::tier::TierSection;
use crate::inputs::SiteInputs;
use crate::pages::layout::Layout;
use crate::pages::shared::{NamedLink, entry_link, movement_note, order_text, strongest_first};

/// The page of one entry.
#[derive(Debug, Template)]
#[template(path = "project.html")]
pub(crate) struct ProjectPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// Name, kind, tier and notices.
    pub header: Header<'a>,
    /// The facts a person reads first.
    pub glance: Glance,
    /// The description with its source.
    pub description: DescriptionView<'a>,
    /// The repo, homepage, crate and install channel links.
    pub links: Vec<LinkRow>,
    /// The tier reasons and signals. `None` without a report.
    pub tier: Option<TierSection<'a>>,
    /// Every `correctness` judgment, newest first.
    pub corrections: Vec<CorrectionRow<'a>>,
    /// Every judgment, newest first.
    pub judgments: Vec<JudgmentRow<'a>>,
    /// Other entries that share a topic, strongest first.
    pub also_in: Vec<AlsoInRow<'a>>,
    /// The entries that name this one in `part_of`, strongest first.
    pub related: Vec<NamedLink<'a>>,
    /// The sentence that states the order of the lists of this page.
    pub order: &'static str,
    /// The line shown when the history cannot measure movement.
    pub movement_note: Option<&'static str>,
}

impl<'a> ProjectPage<'a> {
    /// The page of `effective` in `layout`.
    pub(crate) fn new(
        layout: Layout,
        inputs: &SiteInputs<'a>,
        effective: &EffectiveEntry<'a>,
    ) -> Self {
        let catalogue = inputs.catalogue;
        let report = inputs.report;
        let found = report.and_then(|report| {
            let record = report.entries.get(effective.slug)?;
            Some((report, record))
        });
        Self {
            layout,
            header: Header::new(inputs, effective, found.map(|(_, record)| record)),
            glance: Glance::new(effective.entry, found.map(|(_, record)| record)),
            description: DescriptionView::of(effective.entry),
            links: link_rows(effective.entry),
            tier: found.map(|(report, record)| TierSection::new(report, record)),
            corrections: effective
                .corrections
                .iter()
                .map(CorrectionRow::of)
                .collect(),
            judgments: effective
                .judgments
                .iter()
                .map(|&(issue, judgment)| JudgmentRow::new(catalogue, issue, judgment))
                .collect(),
            also_in: also_in(inputs, effective),
            related: strongest_first(catalogue, report, catalogue.related_to(effective.slug))
                .into_iter()
                .filter_map(|slug| entry_link(catalogue, slug))
                .collect(),
            order: order_text(report),
            movement_note: movement_note(inputs),
        }
    }
}
