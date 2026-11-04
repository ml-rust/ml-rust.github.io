//! The text report of `catalogue github facts`.

use catalogue_signals::github::{CiState, Facts, LATEST_SAMPLE, RepoFacts, RepoRef};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::render::{NONE, or_none};

/// The UTC date of a timestamp, as `YYYY-MM-DD`.
fn day(when: Timestamp) -> String {
    when.to_zoned(TimeZone::UTC).date().to_string()
}

/// The `key: value` lines of one repo, with no trailing newline.
///
/// The keys are `repo`, `status`, `archived`, `fork`, `stars`, `forks`, `commits`, `pushed`,
/// `ci`, `last_human_commit`, `human_commits_365`, `oldest_unanswered_days`,
/// `maintainer_replies_365`, `median_first_response_days` and `items_sampled`.
/// `repo` holds the requested name. `status` is `found`, `gone` or
/// `moved:<new name>`. A `+` follows `human_commits_365` when the count is a
/// lower bound. `last_human_commit` is the date of the newest human commit,
/// however old, or `>30 bot commits` when the newest 30 commits are all bots
/// and the last year holds no human commit. A fact that does not exist prints
/// as `-`.
pub fn render_facts(requested: &RepoRef, facts: &RepoFacts) -> String {
    let rows: Vec<(&str, String)> = match facts {
        RepoFacts::Gone { .. } => [
            "archived",
            "fork",
            "stars",
            "forks",
            "commits",
            "pushed",
            "ci",
            "last_human_commit",
            "human_commits_365",
            "oldest_unanswered_days",
            "maintainer_replies_365",
            "median_first_response_days",
            "items_sampled",
        ]
        .into_iter()
        .map(|key| (key, NONE.to_owned()))
        .collect(),
        RepoFacts::Found(found) => found_rows(found),
    };
    let status = match facts {
        RepoFacts::Found(Facts {
            moved: true,
            name_with_owner,
            ..
        }) => format!("moved:{name_with_owner}"),
        RepoFacts::Found(_) => "found".to_owned(),
        RepoFacts::Gone { .. } => "gone".to_owned(),
    };
    let mut lines = vec![format!("repo: {requested}"), format!("status: {status}")];
    lines.extend(
        rows.into_iter()
            .map(|(key, value)| format!("{key}: {value}")),
    );
    lines.join("\n")
}

/// The rows after `status` for an existing repo.
fn found_rows(facts: &Facts) -> Vec<(&'static str, String)> {
    let last_human = if facts.last_human_commit_beyond_sample {
        format!(">{LATEST_SAMPLE} bot commits")
    } else {
        or_none(facts.last_human_commit)
    };
    let capped = if facts.human_commits_capped { "+" } else { "" };
    vec![
        ("archived", facts.archived.to_string()),
        ("fork", facts.fork.to_string()),
        ("stars", facts.stars.to_string()),
        ("forks", or_none(facts.forks)),
        ("commits", or_none(facts.commits)),
        ("pushed", or_none(facts.pushed_at.map(day))),
        ("ci", or_none(facts.ci.map(CiState::as_str))),
        ("last_human_commit", last_human),
        (
            "human_commits_365",
            format!("{}{capped}", facts.human_commits_365),
        ),
        (
            "oldest_unanswered_days",
            or_none(facts.oldest_unanswered_days),
        ),
        (
            "maintainer_replies_365",
            facts.maintainer_replies_365.to_string(),
        ),
        (
            "median_first_response_days",
            or_none(facts.median_first_response_days),
        ),
        ("items_sampled", facts.items_sampled.to_string()),
    ]
}

/// Renders every repo, with a blank line between repos.
pub fn render_all(results: &[(RepoRef, RepoFacts)]) -> String {
    results
        .iter()
        .map(|(requested, facts)| render_facts(requested, facts))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use catalogue_signals::github::RepoLicense;
    use jiff::civil::date;

    use super::*;

    fn repo(text: &str) -> RepoRef {
        RepoRef::parse(text).expect("parses")
    }

    fn facts() -> Facts {
        Facts {
            name_with_owner: "a/b".to_owned(),
            moved: false,
            archived: false,
            fork: true,
            pushed_at: "2026-10-26T17:14:42Z".parse().ok(),
            stars: 12,
            forks: Some(3),
            commits: Some(450),
            ci: Some(CiState::Failure),
            license: RepoLicense::NoneStated,
            last_human_commit: Some(date(2026, 10, 1)),
            last_human_commit_beyond_sample: false,
            human_commits_365: 100,
            human_commits_capped: true,
            oldest_unanswered_days: Some(40),
            maintainer_replies_365: 9,
            median_first_response_days: None,
            items_sampled: 87,
        }
    }

    #[test]
    fn a_found_repo_prints_every_key() {
        let text = render_facts(&repo("a/b"), &RepoFacts::Found(facts()));
        assert_eq!(
            text,
            "repo: a/b\nstatus: found\narchived: false\nfork: true\nstars: 12\nforks: 3\ncommits: 450\n\
             pushed: 2026-10-26\nci: failure\nlast_human_commit: 2026-10-01\n\
             human_commits_365: 100+\noldest_unanswered_days: 40\n\
             maintainer_replies_365: 9\nmedian_first_response_days: -\nitems_sampled: 87"
        );
    }

    #[test]
    fn a_moved_repo_names_the_new_location() {
        let mut moved = facts();
        moved.moved = true;
        moved.name_with_owner = "c/d".to_owned();
        moved.human_commits_capped = false;
        moved.ci = None;
        moved.pushed_at = None;
        let text = render_facts(&repo("a/b"), &RepoFacts::Found(moved));
        assert!(text.starts_with("repo: a/b\nstatus: moved:c/d\n"), "{text}");
        assert!(text.contains("\nhuman_commits_365: 100\n"), "{text}");
        assert!(
            text.contains("\nci: -\n") && text.contains("\npushed: -\n"),
            "{text}"
        );
    }

    #[test]
    fn an_all_bot_sample_prints_the_bound() {
        let mut bots = facts();
        bots.last_human_commit = None;
        bots.last_human_commit_beyond_sample = true;
        let text = render_facts(&repo("a/b"), &RepoFacts::Found(bots));
        assert!(
            text.contains("\nlast_human_commit: >30 bot commits\n"),
            "{text}"
        );
        let mut none = facts();
        none.last_human_commit = None;
        let text = render_facts(&repo("a/b"), &RepoFacts::Found(none));
        assert!(text.contains("\nlast_human_commit: -\n"), "{text}");
    }

    #[test]
    fn a_gone_repo_prints_dashes() {
        let requested = repo("a/b");
        let text = render_facts(
            &requested,
            &RepoFacts::Gone {
                requested: requested.clone(),
            },
        );
        assert!(
            text.starts_with("repo: a/b\nstatus: gone\narchived: -\n"),
            "{text}"
        );
        assert!(text.ends_with("\nitems_sampled: -"), "{text}");
        assert_eq!(text.lines().count(), 15);
    }

    #[test]
    fn repos_are_separated_by_a_blank_line() {
        let one = (repo("a/b"), RepoFacts::Found(facts()));
        let two = (
            repo("c/d"),
            RepoFacts::Gone {
                requested: repo("c/d"),
            },
        );
        let text = render_all(&[one, two]);
        let blocks: Vec<&str> = text.split("\n\n").collect();
        assert_eq!(blocks.len(), 2);
        assert!(blocks[1].starts_with("repo: c/d\nstatus: gone"));
        assert_eq!(render_all(&[]), "");
    }
}
