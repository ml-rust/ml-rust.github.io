//! The entry page struct and its template binding.

use askama::Template;
use catalogue_core::{EffectiveEntry, Tier};

use super::action::ActionPanel;
use super::also_in::compare_rows;
use super::compat::Compat;
use super::header::Header;
use super::judgments::{CorrectionRow, JudgmentRow};
use super::links::{CrateRow, DescriptionView, crate_rows};
use super::stats::StatStrip;
use super::tier::TierSection;
use crate::forms::CORRECTNESS_FORM;
use crate::href::SafeHref;
use crate::inputs::SiteInputs;
use crate::pages::layout::Layout;
use crate::pages::shared::{
    EntryRow, NamedLink, entry_link, movement_note, order_text, strongest_first, topic_link,
};

/// The page of one entry.
#[derive(Debug, Template)]
#[template(path = "project.html")]
pub(crate) struct ProjectPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// Name, kind, tier and notices.
    pub header: Header<'a>,
    /// The stat strip. `None` without a report.
    pub stats: Option<StatStrip<'a>>,
    /// The install line and the links to use the project.
    pub action: ActionPanel<'a>,
    /// What the project runs on and needs.
    pub compat: Compat<'a>,
    /// The description with its source.
    pub description: DescriptionView<'a>,
    /// The crates of the entry.
    pub crates: Vec<CrateRow>,
    /// The tier reasons and signals. `None` without a report.
    pub tier: Option<TierSection<'a>>,
    /// Every `correctness` judgment, newest first.
    pub corrections: Vec<CorrectionRow<'a>>,
    /// Every judgment, newest first.
    pub judgments: Vec<JudgmentRow<'a>>,
    /// The form that reports a wrong fact.
    pub correction_href: SafeHref,
    /// This entry and the others that share a topic, strongest first after this entry.
    pub compare: Vec<EntryRow<'a>>,
    /// The topics of the entry that the comparison links in full.
    pub compare_topics: Vec<NamedLink<'a>>,
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
        let entry = effective.entry;
        let found = report.and_then(|report| {
            let record = report.entries.get(effective.slug)?;
            Some((report, record))
        });
        let record = found.map(|(_, record)| record);
        let archived = record.is_some_and(|record| record.tier == Tier::Archived);
        Self {
            layout,
            header: Header::new(inputs, effective, record),
            stats: found.map(|(report, record)| StatStrip::new(entry, report, record)),
            action: ActionPanel::new(entry, archived),
            compat: Compat::new(entry, record),
            description: DescriptionView::of(entry),
            crates: crate_rows(entry),
            tier: found.map(|(report, record)| {
                TierSection::new(report, record, entry.kind, catalogue.thresholds())
            }),
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
            correction_href: CORRECTNESS_FORM.new_issue_href(),
            compare: compare_rows(inputs, effective),
            compare_topics: effective
                .topics
                .iter()
                .filter_map(|id| topic_link(catalogue, id))
                .collect(),
            related: strongest_first(catalogue, report, catalogue.related_to(effective.slug))
                .into_iter()
                .filter_map(|slug| entry_link(catalogue, slug))
                .collect(),
            order: order_text(report),
            movement_note: movement_note(inputs),
        }
    }
}
