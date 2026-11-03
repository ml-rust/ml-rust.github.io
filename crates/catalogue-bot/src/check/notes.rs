//! The submission facts, the demotion rule, the approvers and the skipped checks.

use catalogue_core::{GithubLogin, Tier};

use super::report::{Block, Section, sorted};
use super::submission::{Context, Record, text_section};
use crate::convert::{SlugSource, Submission};
use crate::form::FormKind;

/// The derived slug, the org affiliation, `added` and the PR notes of a submission.
pub(super) fn submit_section(context: &Context<'_>) -> Section {
    let Some(Record::Submit(found)) = &context.record else {
        return text_section("Submission", "Unknown until the form parses.");
    };
    Section {
        title: "Submission",
        blocks: vec![
            Block::Text(facts(found)),
            Block::Text("PR notes. They go in the data PR body, never in the entry:".to_owned()),
            Block::Quoted(pr_notes(found)),
        ],
    }
}

fn facts(found: &Submission) -> String {
    let source = match found.slug_source {
        SlugSource::PrimaryCrate => "It comes from the first crate in `Crates`.",
        SlugSource::RepositoryName => "It comes from the repository name.",
    };
    let affiliation = if found.entry.org_affiliated {
        "yes. The repository is on GitHub under `ml-rust`."
    } else {
        "no. The repository is not on GitHub under `ml-rust`."
    };
    [
        format!("- Slug: `{}`. {source}", found.slug),
        format!("- Org affiliated: {affiliation}"),
        format!("- Added: {}.", found.entry.added),
    ]
    .join("\n")
}

fn pr_notes(found: &Submission) -> Vec<String> {
    let notes = &found.notes;
    let mut lines = vec![
        format!("Reporter: {}", notes.reporter),
        format!("Issue: {}", notes.issue),
        format!("Affiliation: {}", notes.affiliation.sentence()),
    ];
    if notes.evidence_of_use.is_empty() {
        lines.push("Evidence of use: none".to_owned());
    } else {
        lines.push("Evidence of use:".to_owned());
        let links = notes
            .evidence_of_use
            .iter()
            .map(|link| format!("- {link}"))
            .collect();
        lines.extend(sorted(links));
    }
    lines
}

/// The demotion rule: the entry must be Featured now. Sets `blocking` otherwise.
pub(super) fn demotion_section(current: Option<Tier>, blocking: &mut bool) -> Section {
    let text = match current {
        Some(Tier::Featured) => {
            "The entry is `featured` now. The demotion can go ahead.".to_owned()
        }
        Some(tier) => {
            *blocking = true;
            format!(
                "The entry is `{}` now. A demotion needs a `featured` entry.",
                tier.as_str()
            )
        }
        None => {
            *blocking = true;
            "Current tier unknown. The check needs it to confirm the entry is `featured`."
                .to_owned()
        }
    };
    text_section("Demotion", &text)
}

/// The maintainers who can approve the record, sorted by login.
pub(super) fn approvers_section(context: &Context<'_>) -> Section {
    let text = match (&context.main, &context.record) {
        (_, None) => "Unknown until the form parses.".to_owned(),
        (Err(_), Some(_)) => "Unknown, because main does not load.".to_owned(),
        (Ok(catalogue), Some(record)) => {
            let reporter = &context.input.meta.author;
            let found = match record {
                Record::Submit(submission) => catalogue.submission_approvers(
                    submission.slug.as_str(),
                    submission.entry.org_affiliated,
                    reporter,
                ),
                Record::Judgment(draft) => {
                    catalogue.eligible_approvers(&draft.entry, reporter, &draft.judgment)
                }
            };
            approver_listing(found)
        }
    };
    text_section("Approvers", &text)
}

/// The sentence that lists `found` approvers, sorted by login without repeats.
pub(crate) fn approver_listing(mut found: Vec<GithubLogin>) -> String {
    found.sort_by_key(GithubLogin::normalized);
    found.dedup_by(|a, b| a.same_account(b));
    if found.is_empty() {
        return "No eligible approver. No maintainer in `data/maintainers.toml` passes the approval rules for this issue.".to_owned();
    }
    let names: Vec<String> = found.iter().map(|login| format!("`{login}`")).collect();
    format!("Eligible approvers: {}.", names.join(", "))
}

/// The checks this report skips, for `kind`.
pub(super) fn not_checked_section(kind: FormKind) -> Section {
    let mut lines = Vec::new();
    if kind == FormKind::Submit {
        lines.push("- Crate existence on crates.io. It needs the network.");
    }
    if kind == FormKind::Maintainer {
        lines.push(
            "- Ownership of the crate on crates.io or the repository on GitHub. It needs the network.",
        );
    }
    lines.push("- The approval rules for the approver. They run when a maintainer approves.");
    text_section("Not checked", &lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skipped_checks_leave_out_the_link_check() {
        for kind in FormKind::ALL {
            let section = not_checked_section(kind);
            for block in &section.blocks {
                let Block::Text(text) = block else {
                    panic!("{kind:?}: the section holds only bot text");
                };
                assert!(!text.contains("evidence links"), "{kind:?}: {text}");
                assert!(text.contains("The approval rules for the approver."));
            }
        }
    }
}
