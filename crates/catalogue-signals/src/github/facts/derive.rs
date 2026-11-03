//! Assembles [`Facts`] from a parsed repository.

use jiff::Timestamp;
use jiff::civil::Date;

use super::commits::{commit_facts, last_human_commit};
use super::query::window_start;
use super::repo_ref::RepoRef;
use super::response::{Item, Repository};
use super::threads::thread_facts;
use super::types::{CiState, Facts};

/// Maps a GitHub check state to a [`CiState`]. Other values map to `None`.
fn ci_state(state: &str) -> Option<CiState> {
    match state {
        "SUCCESS" => Some(CiState::Success),
        "FAILURE" => Some(CiState::Failure),
        "PENDING" => Some(CiState::Pending),
        "ERROR" => Some(CiState::Error),
        "EXPECTED" => Some(CiState::Expected),
        _ => None,
    }
}

/// Derives the facts of `repo`, requested as `requested`, as of `today`.
pub(super) fn derive_facts(repo: &Repository, requested: &RepoRef, today: Date) -> Facts {
    let target = repo
        .default_branch_ref
        .as_ref()
        .and_then(|branch| branch.target.as_ref());
    let ci = target
        .and_then(|target| target.status_check_rollup.as_ref())
        .and_then(|rollup| ci_state(&rollup.state));
    let commits = target
        .and_then(|target| target.recent.as_ref())
        .map(commit_facts)
        .unwrap_or_default();
    let latest = target.and_then(|target| target.latest.as_ref());
    let last_human = last_human_commit(&commits, latest);
    let items: Vec<&Item> = repo
        .issues
        .iter()
        .chain(repo.pull_requests.iter())
        .flat_map(|connection| connection.items())
        .collect();
    let threads = thread_facts(items, today, window_start(today));
    Facts {
        name_with_owner: repo.name_with_owner.clone(),
        moved: !repo
            .name_with_owner
            .eq_ignore_ascii_case(&requested.to_string()),
        archived: repo.is_archived,
        fork: repo.is_fork,
        pushed_at: repo
            .pushed_at
            .as_deref()
            .and_then(|text| text.parse::<Timestamp>().ok()),
        stars: repo.stargazer_count,
        ci,
        last_human_commit: last_human.date,
        last_human_commit_beyond_sample: last_human.beyond_sample,
        human_commits_365: commits.human_count,
        human_commits_capped: commits.capped,
        oldest_unanswered_days: threads.oldest_unanswered_days,
        maintainer_replies_365: threads.maintainer_replies,
        median_first_response_days: threads.median_first_response_days,
        items_sampled: threads.items_sampled,
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use serde_json::json;

    use super::*;

    fn repository(name: &str) -> Repository {
        serde_json::from_value(json!({
            "nameWithOwner": name,
            "isArchived": true,
            "isFork": false,
            "pushedAt": "2026-10-26T17:14:42Z",
            "stargazerCount": 7,
            "defaultBranchRef": null,
            "issues": {"nodes": []},
            "pullRequests": null,
        }))
        .expect("repository")
    }

    #[test]
    fn every_check_state_maps() {
        assert_eq!(ci_state("SUCCESS"), Some(CiState::Success));
        assert_eq!(ci_state("FAILURE"), Some(CiState::Failure));
        assert_eq!(ci_state("PENDING"), Some(CiState::Pending));
        assert_eq!(ci_state("ERROR"), Some(CiState::Error));
        assert_eq!(ci_state("EXPECTED"), Some(CiState::Expected));
        assert_eq!(ci_state("NEW_STATE"), None);
    }

    #[test]
    fn a_repo_without_a_branch_or_items_has_empty_facts() {
        let requested = RepoRef::parse("a/b").expect("parses");
        let facts = derive_facts(&repository("a/b"), &requested, date(2026, 11, 3));
        assert!(!facts.moved && facts.archived && !facts.fork);
        assert_eq!(facts.stars, 7);
        assert_eq!(facts.ci, None);
        assert_eq!(facts.human_commits_365, 0);
        assert_eq!(facts.items_sampled, 0);
        assert!(facts.pushed_at.is_some());
    }

    #[test]
    fn a_case_only_difference_is_not_a_move() {
        let requested = RepoRef::parse("a/b").expect("parses");
        let same = derive_facts(&repository("A/B"), &requested, date(2026, 11, 3));
        assert!(!same.moved);
        let moved = derive_facts(&repository("c/b"), &requested, date(2026, 11, 3));
        assert!(moved.moved);
    }
}
