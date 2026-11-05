//! Crates grouped by GitHub repo.

use std::collections::BTreeMap;

use catalogue_core::normalize_repo_url;

use crate::discover::{Candidate, Reason};
use crate::github::RepoRef;
use crate::seed::ExistingIndex;

/// The host of the repos the list covers.
const GITHUB_HOST: &str = "github.com";

/// The crates of one GitHub repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The repo in lowercase, as the first crate links it.
    pub repo: RepoRef,
    /// The repo as [`normalize_repo_url`] spells it: `github.com/owner/name`.
    pub key: String,
    /// The crate names, sorted.
    pub crates: Vec<String>,
    /// The terms the crates matched, the most crates first, then by text.
    ///
    /// A term is the keyword, the category slug or the vocabulary term of a [`Reason`]. A
    /// dependency on a core crate is no term.
    pub terms: Vec<String>,
}

/// The groups of one run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grouped {
    /// The crates below the floor with a match other than a dependency.
    pub crates: usize,
    /// The repos of those crates, sorted by key.
    pub groups: Vec<Group>,
}

/// Groups the candidates below the floor by GitHub repo.
///
/// A candidate counts when its active users, as `users` gives them, are below `floor`, and at
/// least one [`Reason`] is no dependency on a core crate. A count of exactly `floor` is not
/// below. `repository_of` gives the `repository` link of a crate id. A crate with no GitHub
/// link counts but joins no group. Two links that differ only in case, a `.git` suffix or a
/// trailing path join one group.
pub fn group_repos<'a>(
    candidates: &[Candidate],
    repository_of: impl Fn(u32) -> Option<&'a str>,
    users: impl Fn(u32) -> f64,
    floor: f64,
) -> Grouped {
    let mut counted = 0;
    let mut by_key: BTreeMap<String, Building> = BTreeMap::new();
    for candidate in candidates {
        if users(candidate.crate_id) >= floor {
            continue;
        }
        let texts: Vec<String> = candidate.reasons.iter().filter_map(term_text).collect();
        if texts.is_empty() {
            continue;
        }
        counted += 1;
        let Some((key, repo)) = repository_of(candidate.crate_id).and_then(github_repo) else {
            continue;
        };
        let building = by_key.entry(key).or_insert_with(|| Building {
            repo,
            crates: Vec::new(),
            counts: BTreeMap::new(),
        });
        building.crates.push(candidate.name.clone());
        for text in texts {
            *building.counts.entry(text).or_insert(0) += 1;
        }
    }
    let groups = by_key
        .into_iter()
        .map(|(key, building)| building.finish(key))
        .collect();
    Grouped {
        crates: counted,
        groups,
    }
}

/// Removes the groups whose repo an entry holds, `discovery.toml` excludes or a lead names, and
/// the groups with a crate an entry lists, whatever repo that entry links.
///
/// The index holds each repo as [`normalize_repo_url`] spells it, so case does not matter.
pub fn drop_known(groups: Vec<Group>, known: &ExistingIndex) -> Vec<Group> {
    groups
        .into_iter()
        .filter(|group| {
            !is_known(known, &group.key) && !group.crates.iter().any(|c| known.lists_crate(c))
        })
        .collect()
}

/// Whether `key` is an entry repo, an excluded repo or a lead.
pub(super) fn is_known(known: &ExistingIndex, key: &str) -> bool {
    known.repo_slug(key).is_some() || known.is_excluded(key)
}

struct Building {
    repo: RepoRef,
    crates: Vec<String>,
    counts: BTreeMap<String, usize>,
}

impl Building {
    fn finish(mut self, key: String) -> Group {
        self.crates.sort();
        let mut ranked: Vec<(String, usize)> = self.counts.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Group {
            repo: self.repo,
            key,
            crates: self.crates,
            terms: ranked.into_iter().map(|(text, _)| text).collect(),
        }
    }
}

/// The text of a reason that names a match, `None` for a dependency match.
fn term_text(reason: &Reason) -> Option<String> {
    match reason {
        Reason::Core => Some("core".to_owned()),
        Reason::Named => Some("named".to_owned()),
        Reason::Category(text) | Reason::Keyword(text) | Reason::Term(text) => Some(text.clone()),
        Reason::DependsOn(_) => None,
    }
}

/// The key and the lowercase repo of a GitHub link, `None` for any other link.
fn github_repo(link: &str) -> Option<(String, RepoRef)> {
    let key = normalize_repo_url(link)?;
    let path = key.strip_prefix(GITHUB_HOST)?.strip_prefix('/')?;
    let repo = RepoRef::parse(path).ok()?;
    Some((key, repo))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn candidate(id: u32, name: &str, reasons: Vec<Reason>) -> Candidate {
        Candidate {
            crate_id: id,
            name: name.to_owned(),
            reasons,
            deprecated: false,
        }
    }

    fn keyword(text: &str) -> Reason {
        Reason::Keyword(text.to_owned())
    }

    /// Groups `found` against the `links` of crate ids and the `active` users of crate ids.
    fn run(
        found: &[Candidate],
        links: &[(u32, &str)],
        active: &[(u32, f64)],
        floor: f64,
    ) -> Grouped {
        let links: HashMap<u32, &str> = links.iter().copied().collect();
        let active: HashMap<u32, f64> = active.iter().copied().collect();
        group_repos(
            found,
            |id| links.get(&id).copied(),
            |id| active.get(&id).copied().unwrap_or(0.0),
            floor,
        )
    }

    #[test]
    fn crates_of_one_repo_share_a_group_with_ranked_terms() {
        let found = [
            candidate(1, "zeta", vec![keyword("llm"), keyword("rag")]),
            candidate(
                2,
                "alpha",
                vec![keyword("llm"), Reason::Term("vector search".to_owned())],
            ),
            candidate(3, "other", vec![Reason::Category("science::ml".to_owned())]),
        ];
        let links = [
            (1, "https://github.com/Owner/Repo"),
            (2, "https://github.com/owner/repo.git"),
            (3, "https://github.com/x/y/tree/main"),
        ];
        let grouped = run(&found, &links, &[], 2.0);
        assert_eq!(grouped.crates, 3);
        assert_eq!(grouped.groups.len(), 2);
        let first = &grouped.groups[0];
        assert_eq!(first.key, "github.com/owner/repo");
        assert_eq!(first.repo.to_string(), "owner/repo");
        assert_eq!(first.crates, ["alpha", "zeta"]);
        assert_eq!(first.terms, ["llm", "rag", "vector search"]);
        let second = &grouped.groups[1];
        assert_eq!(second.key, "github.com/x/y");
        assert_eq!(second.terms, ["science::ml"]);
    }

    #[test]
    fn the_floor_boundary_keeps_below_and_drops_at_or_over() {
        let found = [
            candidate(1, "under", vec![keyword("llm")]),
            candidate(2, "exact", vec![keyword("llm")]),
            candidate(3, "over", vec![keyword("llm")]),
        ];
        let links = [
            (1, "https://github.com/a/under"),
            (2, "https://github.com/a/exact"),
            (3, "https://github.com/a/over"),
        ];
        let grouped = run(&found, &links, &[(1, 1.99), (2, 2.0), (3, 7.0)], 2.0);
        assert_eq!(grouped.crates, 1);
        assert_eq!(grouped.groups.len(), 1);
        assert_eq!(grouped.groups[0].crates, ["under"]);
    }

    #[test]
    fn a_dependency_only_match_is_left_out_and_a_mixed_match_stays() {
        let found = [
            candidate(1, "dep-only", vec![Reason::DependsOn("burn".to_owned())]),
            candidate(
                2,
                "mixed",
                vec![keyword("llm"), Reason::DependsOn("burn".to_owned())],
            ),
        ];
        let links = [
            (1, "https://github.com/a/dep"),
            (2, "https://github.com/a/mixed"),
        ];
        let grouped = run(&found, &links, &[], 2.0);
        assert_eq!(grouped.crates, 1);
        assert_eq!(grouped.groups.len(), 1);
        assert_eq!(
            grouped.groups[0].terms,
            ["llm"],
            "the dependency is no term"
        );
    }

    #[test]
    fn core_and_named_reasons_are_terms() {
        let found = [candidate(1, "c", vec![Reason::Core, Reason::Named])];
        let grouped = run(&found, &[(1, "https://github.com/a/c")], &[], 2.0);
        assert_eq!(grouped.groups[0].terms, ["core", "named"]);
    }

    #[test]
    fn a_crate_without_a_github_link_counts_and_joins_no_group() {
        let found = [
            candidate(1, "nolink", vec![keyword("llm")]),
            candidate(2, "forge", vec![keyword("llm")]),
            candidate(3, "bad-name", vec![keyword("llm")]),
        ];
        let links = [
            (2, "https://codeberg.org/a/b"),
            (3, "https://github.com/a/b c"),
        ];
        let grouped = run(&found, &links, &[], 2.0);
        assert_eq!(grouped.crates, 3);
        assert!(grouped.groups.is_empty());
    }

    #[test]
    fn terms_with_equal_counts_sort_by_text() {
        let found = [candidate(
            1,
            "c",
            vec![keyword("zz"), keyword("aa"), keyword("mm")],
        )];
        let grouped = run(&found, &[(1, "https://github.com/a/c")], &[], 2.0);
        assert_eq!(grouped.groups[0].terms, ["aa", "mm", "zz"]);
    }

    #[test]
    fn a_repo_with_a_crate_an_entry_lists_is_known_under_another_owner() {
        let found = [
            candidate(1, "listed", vec![keyword("llm")]),
            candidate(2, "fresh", vec![keyword("llm")]),
        ];
        let links = [
            (1, "https://github.com/moved/here"),
            (2, "https://github.com/new/repo"),
        ];
        let groups = run(&found, &links, &[], 2.0).groups;
        let known = ExistingIndex::new(["listed"], []);
        let kept = drop_known(groups, &known);
        let keys: Vec<&str> = kept.iter().map(|group| group.key.as_str()).collect();
        assert_eq!(keys, ["github.com/new/repo"]);
    }

    #[test]
    fn known_repos_are_dropped_in_any_case() {
        let found = [
            candidate(1, "entry", vec![keyword("llm")]),
            candidate(2, "excluded", vec![keyword("llm")]),
            candidate(3, "lead", vec![keyword("llm")]),
            candidate(4, "fresh", vec![keyword("llm")]),
        ];
        let links = [
            (1, "https://github.com/Held/Entry"),
            (2, "https://github.com/Skip/Me"),
            (3, "https://github.com/LEAD/Repo"),
            (4, "https://github.com/new/repo"),
        ];
        let groups = run(&found, &links, &[], 2.0).groups;
        let known = ExistingIndex::default()
            .with_repo("github.com/held/entry", "entry")
            .with_excluded("github.com/skip/me")
            .with_repo("github.com/lead/repo", "lead");
        let kept = drop_known(groups, &known);
        let keys: Vec<&str> = kept.iter().map(|group| group.key.as_str()).collect();
        assert_eq!(keys, ["github.com/new/repo"]);
    }
}
