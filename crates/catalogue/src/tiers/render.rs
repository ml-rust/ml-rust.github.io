//! The text report of `catalogue tiers`.

use super::assess::Assessment;
use crate::render::{NONE, or_none};

/// The `catalogue tiers` report: an `entries: <n>` header, then one block per
/// assessment, a blank line between blocks.
///
/// A block holds the lines `slug`, `tier`, `stable`, `dead_now` and
/// `demotion_pending`, then `reasons:` with one indented `rule: detail` line
/// per reason, then `unknown` and `notes` as comma-separated lists. An empty
/// value or list prints as `-`.
pub(crate) fn render_tiers(assessments: &[Assessment]) -> String {
    let mut blocks = vec![format!("entries: {}", assessments.len())];
    blocks.extend(assessments.iter().map(render_entry));
    blocks.join("\n\n")
}

fn render_entry(assessment: &Assessment) -> String {
    let verdict = &assessment.verdict;
    let mut lines = vec![
        format!("slug: {}", assessment.slug),
        format!("tier: {}", verdict.tier.as_str()),
        format!("stable: {}", verdict.stable),
        format!("dead_now: {}", verdict.dead_now),
        format!(
            "demotion_pending: {}",
            or_none(verdict.demotion_pending.map(|tier| tier.as_str()))
        ),
    ];
    if verdict.reasons.is_empty() {
        lines.push(format!("reasons: {NONE}"));
    } else {
        lines.push("reasons:".to_owned());
        lines.extend(
            verdict
                .reasons
                .iter()
                .map(|reason| format!("  {}: {}", reason.rule.as_str(), reason.detail)),
        );
    }
    lines.push(format!("unknown: {}", list(&verdict.unknown)));
    lines.push(format!("notes: {}", list(&assessment.notes)));
    lines.join("\n")
}

/// The items joined with `, `, or [`NONE`] for an empty list.
fn list<T: AsRef<str>>(items: &[T]) -> String {
    if items.is_empty() {
        NONE.to_owned()
    } else {
        let texts: Vec<&str> = items.iter().map(|item| item.as_ref()).collect();
        texts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::{EntryKind, Reason, Rule, Slug, Tier, Verdict};
    use catalogue_signals::history::Row;

    use super::*;

    fn assessment(slug: &str, verdict: Verdict, notes: &[&str]) -> Assessment {
        let row = Row {
            kind: EntryKind::Library,
            tier: verdict.tier,
            dead_now: verdict.dead_now,
            active_users: None,
            docker_pulls: None,
        };
        Assessment {
            slug: Slug::new(slug).expect("slug"),
            verdict,
            row,
            notes: notes.iter().map(|note| (*note).to_owned()).collect(),
        }
    }

    #[test]
    fn renders_a_header_and_one_block_per_entry() {
        let held = Verdict {
            tier: Tier::Featured,
            stable: true,
            dead_now: true,
            reasons: vec![
                Reason::new(Rule::Dormancy, "last activity 2020-01-01: dormant"),
                Reason::new(Rule::DemotionPending, "held"),
            ],
            unknown: vec!["active_users", "repo_gone"],
            demotion_pending: Some(Tier::Archived),
        };
        let plain = Verdict {
            tier: Tier::Listed,
            stable: false,
            dead_now: false,
            reasons: Vec::new(),
            unknown: Vec::new(),
            demotion_pending: None,
        };
        let text = render_tiers(&[
            assessment("alpha", held, &["crate a is not in the dump", "x"]),
            assessment("beta", plain, &[]),
        ]);
        let expected = "entries: 2\n\n\
            slug: alpha\ntier: featured\nstable: true\ndead_now: true\n\
            demotion_pending: archived\nreasons:\n\
            \x20 dormancy: last activity 2020-01-01: dormant\n\
            \x20 demotion-pending: held\n\
            unknown: active_users, repo_gone\n\
            notes: crate a is not in the dump, x\n\n\
            slug: beta\ntier: listed\nstable: false\ndead_now: false\n\
            demotion_pending: -\nreasons: -\nunknown: -\nnotes: -";
        assert_eq!(text, expected);
    }

    #[test]
    fn no_entries_prints_the_header_only() {
        assert_eq!(render_tiers(&[]), "entries: 0");
    }
}
