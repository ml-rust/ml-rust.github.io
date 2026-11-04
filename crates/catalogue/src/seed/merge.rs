//! The report of a run that drafts from a lead list and from discovery.

use catalogue_signals::discover::Rules;
use catalogue_signals::github::RepoRef;
use catalogue_signals::http::Transport;
use catalogue_signals::seed::{LeadInputs, LeadPacers};

use super::build::build_seed;
use super::error::SeedError;
use super::leads::{LeadSeed, build_lead_seed};

/// Drafts entries for `leads`, then for the candidates of `rules`, and merges both.
///
/// The lead run comes first and discovery reads its drafts as entries. So a crate of a lead draft
/// drops out of discovery, and a family with the repository or the slug of a lead draft goes to
/// the skips. One crate, slug or repository never appears in two drafts. The report holds
/// `total drafts: <n>`, then the lead report under `leads:` and the discovery report under
/// `discovery:`.
///
/// # Errors
///
/// Returns [`SeedError::Github`] for a 401 answer. Every other failed fetch stays on its lead.
pub fn build_merged_seed(
    inputs: &LeadInputs<'_>,
    rules: &Rules,
    leads: &[RepoRef],
    transport: &dyn Transport,
    token: &str,
    pacers: &mut LeadPacers<'_>,
) -> Result<LeadSeed, SeedError> {
    let from_leads = build_lead_seed(inputs, leads, transport, token, pacers)?;
    let existing = inputs
        .existing
        .clone()
        .with_drafts(&from_leads.drafts.entries);
    let discovered = build_seed(
        inputs.snapshot,
        rules,
        &inputs.mappings,
        &existing,
        inputs.min_active_users,
    );
    let mut drafts = from_leads.drafts;
    drafts.entries.extend(discovered.drafts.entries);
    drafts.entries.sort_by(|a, b| a.0.cmp(&b.0));
    drafts.skipped.extend(discovered.drafts.skipped);
    drafts.skipped.sort();
    let text = format!(
        "total drafts: {}\nleads:\n{}\ndiscovery:\n{}",
        drafts.entries.len(),
        from_leads.text,
        discovered.text
    );
    Ok(LeadSeed {
        drafts,
        text,
        warnings: from_leads.warnings,
    })
}
