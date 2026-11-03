//! The corrections and the full judgment list of an entry.

use catalogue_core::{Catalogue, Correction, Judgment, JudgmentKind};

use crate::href::SafeHref;
use crate::pages::shared::{
    EvidenceLink, NamedLink, entry_link, project_kind_name, status_name, tier_name, topic_link,
};

/// One `correctness` judgment, as the corrections section shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CorrectionRow<'a> {
    /// The issue number.
    pub issue: u64,
    /// The link to the issue.
    pub issue_href: SafeHref,
    /// The approval date.
    pub date: String,
    /// What the review found.
    pub finding: &'a str,
    /// The links that support the finding.
    pub evidence: Vec<EvidenceLink>,
}

impl<'a> CorrectionRow<'a> {
    /// The row of `correction`.
    pub(crate) fn of(correction: &Correction<'a>) -> Self {
        Self {
            issue: correction.issue,
            issue_href: SafeHref::from(correction.link),
            date: correction.date.to_string(),
            finding: correction.finding,
            evidence: EvidenceLink::all(correction.evidence),
        }
    }
}

/// One judgment, as the judgment table shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JudgmentRow<'a> {
    /// The issue number.
    pub issue: u64,
    /// The link to the issue.
    pub issue_href: SafeHref,
    /// The judgment type.
    pub kind: &'static str,
    /// What the judgment decides, before `links`.
    pub decision: String,
    /// The entries or topics the decision names.
    pub links: Vec<NamedLink<'a>>,
    /// The approval date.
    pub date: String,
    /// Who raised the issue.
    pub reporter: &'a str,
    /// The maintainer who approved it.
    pub approver: &'a str,
    /// The links that support the decision.
    pub evidence: Vec<EvidenceLink>,
}

impl<'a> JudgmentRow<'a> {
    /// The row of `judgment`, raised in issue `issue`.
    pub(crate) fn new(catalogue: &'a Catalogue, issue: u64, judgment: &'a Judgment) -> Self {
        let (kind, decision, links) = decision(catalogue, &judgment.judgment);
        Self {
            issue,
            issue_href: SafeHref::from(&judgment.issue),
            kind,
            decision,
            links,
            date: judgment.date.to_string(),
            reporter: judgment.reporter.as_str(),
            approver: judgment.approver.as_str(),
            evidence: EvidenceLink::all(&judgment.evidence),
        }
    }
}

/// The type name, the decision text and the linked entries or topics of `kind`.
fn decision<'a>(
    catalogue: &'a Catalogue,
    kind: &'a JudgmentKind,
) -> (&'static str, String, Vec<NamedLink<'a>>) {
    match kind {
        JudgmentKind::Correctness { finding } => ("Correctness", finding.clone(), Vec::new()),
        JudgmentKind::Supersedes { by } => (
            "Supersedes",
            "Replaced by".to_owned(),
            entry_link(catalogue, by).into_iter().collect(),
        ),
        JudgmentKind::Kind { value } => ("Kind", project_kind_name(*value).to_owned(), Vec::new()),
        JudgmentKind::Category { topics } => (
            "Category",
            "Topics:".to_owned(),
            topics
                .iter()
                .filter_map(|id| topic_link(catalogue, id))
                .collect(),
        ),
        JudgmentKind::Demotion { from, to } => (
            "Demotion",
            format!("{} to {}", tier_name(*from), tier_name(*to)),
            Vec::new(),
        ),
        JudgmentKind::Maintainer { status } => (
            "Maintainer statement",
            status_name(*status).to_owned(),
            Vec::new(),
        ),
        JudgmentKind::FeaturedApproval {} => (
            "Featured approval",
            "A maintainer outside the organization approves Featured.".to_owned(),
            Vec::new(),
        ),
    }
}
