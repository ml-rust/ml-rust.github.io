//! The lead file and the report of `catalogue seed --leads`.

use std::fs;
use std::path::Path;

use catalogue_core::{Entry, EntryKind, Slug};
use catalogue_signals::github::RepoRef;
use catalogue_signals::http::Transport;
use catalogue_signals::seed::{Drafts, LeadDrafts, LeadInputs, LeadPacers, seed_leads};

use super::error::SeedError;

/// What the report says about every draft and skip of a lead run.
const BYPASS: &str = "A lead bypasses the discovery rules and the active-user floor. Tier rules handle small projects.";

/// The first line of the `catalogue discover github` report.
const DISCOVER_HEADER: &str = "github leads:";

/// The result of one lead run.
#[derive(Debug, PartialEq, Eq)]
pub struct LeadSeed {
    /// The drafts and the skips.
    pub drafts: Drafts,
    /// The report: the libraries, the apps, the listed leads, then the skips.
    pub text: String,
    /// Channel lookups that failed.
    pub warnings: Vec<String>,
}

/// Reads the lead file at `path`.
pub(super) fn read_leads(path: &Path) -> Result<Vec<RepoRef>, SeedError> {
    let text = fs::read_to_string(path).map_err(|source| SeedError::ReadLeads {
        path: path.to_path_buf(),
        source,
    })?;
    parse_leads(path, &text)
}

/// Parses a lead list. `path` only names the file in an error.
///
/// A line holds `owner/repo`. A tab ends the name, so the report of `catalogue discover github`
/// reads as a lead list, without its header. A blank line and a line that starts with `#` hold
/// no lead.
///
/// # Errors
///
/// Returns [`SeedError::BadLead`] for the first line that is not a repo, and
/// [`SeedError::NoLeads`] when no line names one.
pub fn parse_leads(path: &Path, text: &str) -> Result<Vec<RepoRef>, SeedError> {
    let mut leads = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_text = line.trim();
        if line_text.is_empty()
            || line_text.starts_with('#')
            || line_text.starts_with(DISCOVER_HEADER)
        {
            continue;
        }
        let name = line_text.split('\t').next().unwrap_or(line_text);
        let lead = RepoRef::parse(name).map_err(|_| SeedError::BadLead {
            path: path.to_path_buf(),
            line: index + 1,
            input: name.to_owned(),
        })?;
        leads.push(lead);
    }
    if leads.is_empty() {
        return Err(SeedError::NoLeads {
            path: path.to_path_buf(),
        });
    }
    Ok(leads)
}

/// Drafts entries for `leads` and renders the report.
///
/// The report holds `drafts: <n>`, then `libraries: <n>`, `apps: <n>`, `listed: <n>` and
/// `skipped: <n>`, each followed by its lines. A draft line holds the slug, the highest active
/// users of its family with 2 decimals, the comma-joined topics, `lead:<lead>` and the labels. A
/// listed line holds the lead and the entry that covers it. A skip line holds the name and the
/// reason. Fields separate with a tab. Repository text stays out, because it is untrusted.
///
/// # Errors
///
/// Returns [`SeedError::Github`] for a 401 answer. Every other failed fetch stays on its lead.
pub fn build_lead_seed(
    inputs: &LeadInputs<'_>,
    leads: &[RepoRef],
    transport: &dyn Transport,
    token: &str,
    pacers: &mut LeadPacers<'_>,
) -> Result<LeadSeed, SeedError> {
    let found = seed_leads(inputs, leads, transport, token, pacers)?;
    let text = render(&found);
    Ok(LeadSeed {
        drafts: found.drafts,
        text,
        warnings: found.warnings,
    })
}

fn render(found: &LeadDrafts) -> String {
    let entries = &found.drafts.entries;
    let mut lines = vec![format!("drafts: {}. {BYPASS}", entries.len())];
    for (label, kind) in [("libraries", EntryKind::Library), ("apps", EntryKind::App)] {
        let group: Vec<&(Slug, Entry)> = entries.iter().filter(|(_, e)| e.kind == kind).collect();
        lines.push(format!("{label}: {}", group.len()));
        lines.extend(
            group
                .iter()
                .map(|(slug, entry)| draft_line(found, slug, entry)),
        );
    }
    lines.push(format!("listed: {}", found.listed.len()));
    for (lead, text) in &found.listed {
        lines.push(format!("{lead}\t{text}"));
    }
    lines.push(format!("skipped: {}. {BYPASS}", found.drafts.skipped.len()));
    for (name, reason) in &found.drafts.skipped {
        lines.push(format!("{name}\t{reason}"));
    }
    lines.join("\n")
}

fn draft_line(found: &LeadDrafts, slug: &Slug, entry: &Entry) -> String {
    let topics: Vec<&str> = entry.topics.iter().map(Slug::as_str).collect();
    let mut fields = Vec::new();
    match found.details.get(slug) {
        Some(detail) => {
            fields.push(format!("{slug}\t{:.2}\t{}", detail.users, topics.join(",")));
            fields.push(format!("lead:{}", detail.lead));
            fields.extend(detail.notes.iter().cloned());
        }
        None => fields.push(format!("{slug}\t0.00\t{}", topics.join(","))),
    }
    fields.join("\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Vec<String>, SeedError> {
        let leads = parse_leads(Path::new("leads.txt"), text)?;
        Ok(leads.iter().map(ToString::to_string).collect())
    }

    #[test]
    fn a_plain_list_keeps_its_order() {
        assert_eq!(
            parse("b/two\na/one.rs\n\n# a note\n  c/three  \n").expect("parses"),
            ["b/two", "a/one.rs", "c/three"]
        );
    }

    #[test]
    fn a_discover_report_reads_as_a_lead_list() {
        let report = "github leads: 2\nz/top\t90\t2026-01-02\ttopics:llm\na/low\t20\t\ttopics:llm";
        assert_eq!(parse(report).expect("parses"), ["z/top", "a/low"]);
    }

    #[test]
    fn a_bad_line_names_its_number_and_text() {
        let error = parse("a/one\nnot a repo\n").expect_err("bad line");
        assert!(
            matches!(&error, SeedError::BadLead { line: 2, input, .. } if input == "not a repo"),
            "{error}"
        );
    }

    #[test]
    fn a_list_without_a_lead_is_an_error() {
        for text in ["", "\n# only a note\n", "github leads: 0\n"] {
            let error = parse(text).expect_err("no leads");
            assert!(matches!(error, SeedError::NoLeads { .. }), "{error}");
        }
    }

    #[test]
    fn a_missing_file_names_the_path() {
        let error = read_leads(Path::new("/nonexistent/leads.txt")).expect_err("missing");
        assert!(matches!(error, SeedError::ReadLeads { .. }), "{error}");
        assert!(
            error.to_string().contains("/nonexistent/leads.txt"),
            "{error}"
        );
    }
}
