//! The entry page struct and its template binding.

use askama::Template;
use catalogue_core::{Catalogue, EffectiveEntry, TierReport};

use super::also_in::{AlsoInRow, also_in};
use super::glance::Glance;
use super::header::Header;
use super::judgments::{CorrectionRow, JudgmentRow};
use super::links::{DescriptionView, LinkRow, link_rows};
use super::tier::TierSection;
use crate::pages::layout::Layout;

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
}

impl<'a> ProjectPage<'a> {
    /// The page of `effective` in `layout`, with its tier from `report` when given.
    pub(crate) fn new(
        layout: Layout,
        catalogue: &'a Catalogue,
        report: Option<&'a TierReport>,
        effective: &EffectiveEntry<'a>,
    ) -> Self {
        let found = report.and_then(|report| {
            let record = report.entries.get(effective.slug)?;
            Some((report, record))
        });
        Self {
            layout,
            header: Header::new(catalogue, effective, found.map(|(_, record)| record)),
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
            also_in: also_in(catalogue, report, effective),
        }
    }
}
