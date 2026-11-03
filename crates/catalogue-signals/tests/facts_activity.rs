#[path = "common/facts_responses.rs"]
mod facts_responses;
#[path = "common/facts_setup.rs"]
mod facts_setup;
#[path = "common/search_responses.rs"]
mod search_responses;
#[path = "common/transport.rs"]
mod transport;

use facts_responses::{RepoBody, commit, item};
use facts_setup::found;
use jiff::civil::date;
use serde_json::{Value, json};

/// A comment node.
fn comment(when: &str, association: &str) -> Value {
    json!({"createdAt": when, "authorAssociation": association, "author": {"login": "c"}})
}

fn answered(created: &str, answer: &str) -> Value {
    item(
        created,
        "CLOSED",
        "NONE",
        "u",
        vec![comment(answer, "OWNER")],
    )
}

#[test]
fn bot_commits_are_filtered() {
    let mut body = RepoBody::new("a/b");
    body.total_commits = 5;
    body.commits = vec![
        commit(
            "2026-11-01T00:00:00Z",
            Some("dependabot[bot]"),
            "dependabot[bot]",
        ),
        commit("2026-10-31T00:00:00Z", None, "release-plz[bot]"),
        commit("2026-10-30T00:00:00Z", Some("renovate"), "Renovate Bot"),
        commit("2026-10-29T00:00:00Z", Some("alice"), "Alice"),
        commit("2026-10-01T00:00:00Z", None, "Bob"),
    ];
    let facts = found(&body);
    assert_eq!(facts.human_commits_365, 2);
    assert_eq!(facts.last_human_commit, Some(date(2026, 10, 29)));
}

#[test]
fn a_window_over_one_hundred_commits_is_capped() {
    let mut body = RepoBody::new("a/b");
    body.total_commits = 250;
    body.commits = (0..100)
        .map(|_| commit("2026-10-01T00:00:00Z", Some("alice"), "Alice"))
        .collect();
    let facts = found(&body);
    assert_eq!(facts.human_commits_365, 100);
    assert!(facts.human_commits_capped);
}

#[test]
fn a_repo_with_no_human_commit_has_no_date() {
    let facts = found(&RepoBody::new("a/b"));
    assert_eq!(facts.last_human_commit, None);
    assert_eq!(facts.human_commits_365, 0);
}

#[test]
fn a_human_commit_beyond_the_window_sets_the_date() {
    let mut body = RepoBody::new("a/b");
    body.latest_total = 400;
    body.latest_commits = vec![
        commit(
            "2026-10-30T00:00:00Z",
            Some("dependabot[bot]"),
            "dependabot[bot]",
        ),
        commit("2024-08-27T09:00:00Z", Some("alice"), "Alice"),
    ];
    let facts = found(&body);
    assert_eq!(facts.last_human_commit, Some(date(2024, 8, 27)));
    assert!(!facts.last_human_commit_beyond_sample);
    assert_eq!(facts.human_commits_365, 0);
}

#[test]
fn an_all_bot_latest_sample_sets_the_flag() {
    let mut body = RepoBody::new("a/b");
    body.latest_total = 400;
    body.latest_commits = (0..30)
        .map(|_| {
            commit(
                "2026-10-30T00:00:00Z",
                Some("dependabot[bot]"),
                "dependabot[bot]",
            )
        })
        .collect();
    let facts = found(&body);
    assert_eq!(facts.last_human_commit, None);
    assert!(facts.last_human_commit_beyond_sample);
}

#[test]
fn the_oldest_unanswered_item_sets_the_age() {
    let mut body = RepoBody::new("a/b");
    body.issues = vec![
        item("2026-10-24T00:00:00Z", "OPEN", "NONE", "u1", vec![]),
        item("2026-09-24T00:00:00Z", "OPEN", "FIRST_TIMER", "u2", vec![]),
    ];
    body.pulls = vec![item(
        "2026-01-01T00:00:00Z",
        "OPEN",
        "NONE",
        "u3",
        vec![comment("2026-01-02T00:00:00Z", "MEMBER")],
    )];
    assert_eq!(found(&body).oldest_unanswered_days, Some(40));
}

#[test]
fn only_maintainer_comments_inside_the_window_count_as_replies() {
    let mut body = RepoBody::new("a/b");
    body.issues = vec![item(
        "2025-01-01T00:00:00Z",
        "CLOSED",
        "NONE",
        "u",
        vec![
            comment("2025-11-02T00:00:00Z", "OWNER"),
            comment("2025-11-03T00:00:00Z", "OWNER"),
            comment("2026-10-01T00:00:00Z", "NONE"),
        ],
    )];
    body.pulls = vec![item(
        "2026-10-01T00:00:00Z",
        "OPEN",
        "OWNER",
        "m",
        vec![comment("2026-10-02T00:00:00Z", "COLLABORATOR")],
    )];
    assert_eq!(found(&body).maintainer_replies_365, 2);
}

#[test]
fn the_median_first_response_covers_odd_even_and_none() {
    let mut body = RepoBody::new("a/b");
    assert_eq!(found(&body).median_first_response_days, None);
    body.issues = vec![
        answered("2026-10-01T00:00:00Z", "2026-10-02T00:00:00Z"),
        answered("2026-10-01T00:00:00Z", "2026-10-11T00:00:00Z"),
    ];
    assert_eq!(found(&body).median_first_response_days, Some(1));
    body.pulls = vec![answered("2026-10-01T00:00:00Z", "2026-10-04T00:00:00Z")];
    assert_eq!(found(&body).median_first_response_days, Some(3));
}
