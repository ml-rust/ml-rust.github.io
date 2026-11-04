//! The choice of a family's primary crate.
//!
//! The primary is the main crate of the repository. Use counts do not choose it: a shared
//! sub-crate has more users than the crate of the project. The name of the repository and the
//! family's own dependency tree choose it.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use super::owners::Member;
use crate::discover::{Candidate, text_words};
use crate::dump::{DependencyKind, DumpSnapshot};

/// Name segments of a crate that supports the project and is not the project itself.
const AUXILIARY_SEGMENTS: &[&str] = &[
    "bench",
    "benches",
    "benchmark",
    "benchmarks",
    "bindings",
    "capi",
    "cli",
    "demo",
    "demos",
    "derive",
    "example",
    "examples",
    "ffi",
    "macro",
    "macros",
    "node",
    "nodejs",
    "py",
    "pyo3",
    "python",
    "sys",
    "temp",
    "test",
    "testing",
    "tests",
    "tmp",
    "wasm",
];

/// Language markers that a repository or crate name can carry at either end.
const LANGUAGE_MARKERS: &[&str] = &["rs", "rust"];

/// Words that say nothing about which project a name belongs to.
const FILLER_WORDS: &[&str] = &["rs", "rust", "lib"];

/// The lowercase words of `name`, split at every character that is not a letter or digit.
pub(super) fn words(name: &str) -> Vec<String> {
    text_words(name).map(str::to_lowercase).collect()
}

/// `words` without a language marker at either end, unless the marker is the only word.
fn without_markers(words: &[String]) -> &[String] {
    let is_marker = |word: &String| LANGUAGE_MARKERS.contains(&word.as_str());
    let mut kept = words;
    if kept.len() > 1 && kept.first().is_some_and(is_marker) {
        kept = kept.get(1..).unwrap_or(kept);
    }
    if kept.len() > 1 && kept.last().is_some_and(is_marker) {
        kept = kept.get(..kept.len() - 1).unwrap_or(kept);
    }
    kept
}

/// The first word of `name` after any language marker. Two crates of one project share it.
pub(super) fn lead_word(name: &str) -> Option<String> {
    without_markers(&words(name)).first().cloned()
}

/// How a crate name relates to the name of the repository.
pub(super) struct RepoName {
    joined: String,
    stem: String,
    lead: Option<String>,
    /// The words of the name without filler words. The whole words when only filler remains.
    key_words: Vec<String>,
}

impl RepoName {
    /// Reads the last segment of a repository link.
    pub(super) fn new(segment: &str) -> Self {
        let all = words(segment);
        let kept = without_markers(&all);
        let mut key_words: Vec<String> = all
            .iter()
            .filter(|word| !FILLER_WORDS.contains(&word.as_str()))
            .cloned()
            .collect();
        if key_words.is_empty() {
            key_words.clone_from(&all);
        }
        Self {
            key_words,
            joined: all.concat(),
            stem: kept.concat(),
            lead: kept.first().cloned(),
        }
    }

    /// 2 when `name` is the repository name apart from case and separators, so `mistralrs`
    /// matches `mistral.rs`. 1 when it matches without a language marker, so `tch` matches
    /// `tch-rs`. 0 otherwise.
    fn match_tier(&self, name: &str) -> u8 {
        let all = words(name);
        if all.concat() == self.joined {
            2
        } else if without_markers(&all).concat() == self.stem {
            1
        } else {
            0
        }
    }

    /// Whether crate `name` shares a word with the repository name.
    ///
    /// Filler words (`rust`, `rs`, `lib`) do not count, and `-`, `_` and `.` separate words. A
    /// crate word matches a repository word when it starts with it, so `mistralrs` matches
    /// `mistral.rs`. A crate word with a `lib` prefix matches after the prefix goes, so
    /// `liboxen` matches `oxen`.
    pub(super) fn names_repo(&self, name: &str) -> bool {
        words(name)
            .iter()
            .filter(|word| !FILLER_WORDS.contains(&word.as_str()))
            .any(|word| {
                let bare = word.strip_prefix("lib").filter(|bare| !bare.is_empty());
                self.key_words.iter().any(|key| {
                    word.starts_with(key.as_str())
                        || bare.is_some_and(|bare| bare.starts_with(key.as_str()))
                })
            })
    }

    fn shares_lead(&self, name: &str) -> bool {
        self.lead.is_some() && lead_word(name) == self.lead
    }

    /// Whether crate `name` is named for Rust use of this repository.
    ///
    /// It matches the repository name, shares its first word, or carries a language marker at
    /// either end. A crate that only names the repository link has none of these.
    pub(super) fn is_rust_named(&self, name: &str) -> bool {
        let all = words(name);
        let marked = |word: Option<&String>| {
            word.is_some_and(|word| LANGUAGE_MARKERS.contains(&word.as_str()))
        };
        self.match_tier(name) > 0
            || self.shares_lead(name)
            || marked(all.first())
            || marked(all.last())
    }
}

fn is_auxiliary(name: &str) -> bool {
    words(name)
        .iter()
        .any(|word| AUXILIARY_SEGMENTS.contains(&word.as_str()))
}

/// Picks the owner group that holds the project, and returns it with the other crates.
///
/// The group with the crate named most closely like the repository wins. Then the group with
/// the most active users on one crate, the most crates and the first crate name. `None` when
/// `groups` is empty.
pub(super) fn main_group<'a>(
    repo: &RepoName,
    groups: Vec<Vec<Member<'a>>>,
) -> Option<(Vec<Member<'a>>, Vec<Member<'a>>)> {
    let score = |group: &[Member<'a>]| {
        let tier = group
            .iter()
            .map(|(candidate, _)| repo.match_tier(&candidate.name))
            .max()
            .unwrap_or_default();
        let users = group.iter().map(|(_, users)| *users).fold(0.0, f64::max);
        (tier, users, group.len())
    };
    let mut best: Option<Vec<Member<'a>>> = None;
    let mut others = Vec::new();
    for group in groups {
        match best.take() {
            None => best = Some(group),
            Some(held) => {
                let (held_tier, held_users, held_len) = score(&held);
                let (tier, users, len) = score(&group);
                let wins = tier
                    .cmp(&held_tier)
                    .then(users.total_cmp(&held_users))
                    .then(len.cmp(&held_len))
                    .then_with(|| first_name(&held).cmp(first_name(&group)))
                    == Ordering::Greater;
                let (kept, dropped) = if wins { (group, held) } else { (held, group) };
                best = Some(kept);
                others.extend(dropped);
            }
        }
    }
    best.map(|group| (group, others))
}

fn first_name<'g>(group: &'g [Member<'_>]) -> &'g str {
    group
        .iter()
        .map(|(candidate, _)| candidate.name.as_str())
        .min()
        .unwrap_or_default()
}

/// Sorts `members` so the primary comes first and the crates most like it follow.
///
/// The order, best first:
/// - The match with the repository name, exact before the one without a language marker.
/// - Not auxiliary by name segment.
/// - Same first word as the repository name.
/// - Reach: the most other members it depends on or that depend on it, directly or not. Plain
///   dependents would pick the lowest kernel crate. Plain roots would pick a leaf.
/// - The fewest name segments: an umbrella crate has a shorter name than its parts.
/// - The most dependents: the crate a family builds on comes before a crate on top of it.
/// - The most active users, the shortest name, then the name.
pub(super) fn rank<'a>(
    snapshot: &DumpSnapshot,
    repo: &RepoName,
    members: &[Member<'a>],
) -> Vec<&'a Candidate> {
    let uses = family_edges(snapshot, members);
    let mut used_by: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for (from, targets) in &uses {
        for target in targets {
            used_by.entry(*target).or_default().insert(*from);
        }
    }
    let mut ranked: Vec<Ranked<'a>> = members
        .iter()
        .map(|&(candidate, users)| {
            let dependents = reachable(&used_by, candidate.crate_id);
            let dependencies = reachable(&uses, candidate.crate_id);
            Ranked {
                candidate,
                users,
                tier: repo.match_tier(&candidate.name),
                auxiliary: is_auxiliary(&candidate.name),
                shares_lead: repo.shares_lead(&candidate.name),
                reach: dependents.union(&dependencies).count(),
                segments: words(&candidate.name).len(),
                dependents: dependents.len(),
            }
        })
        .collect();
    ranked.sort_by(Ranked::order);
    ranked.into_iter().map(|r| r.candidate).collect()
}

/// The normal and build requirements of each member's default version on other members.
fn family_edges(snapshot: &DumpSnapshot, members: &[Member<'_>]) -> BTreeMap<u32, BTreeSet<u32>> {
    let ids: BTreeSet<u32> = members.iter().map(|(c, _)| c.crate_id).collect();
    members
        .iter()
        .map(|(candidate, _)| {
            let id = candidate.crate_id;
            let targets: BTreeSet<u32> = snapshot
                .default_requirements
                .get(&id)
                .into_iter()
                .flatten()
                .filter(|req| req.kind != DependencyKind::Dev)
                .map(|req| req.crate_id)
                .filter(|dep| *dep != id && ids.contains(dep))
                .collect();
            (id, targets)
        })
        .collect()
}

/// The ids that `start` reaches in `graph`, without `start`.
fn reachable(graph: &BTreeMap<u32, BTreeSet<u32>>, start: u32) -> BTreeSet<u32> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![start];
    while let Some(id) = stack.pop() {
        for next in graph.get(&id).into_iter().flatten() {
            if *next != start && seen.insert(*next) {
                stack.push(*next);
            }
        }
    }
    seen
}

/// A member with the facts its place in the order depends on.
struct Ranked<'a> {
    candidate: &'a Candidate,
    users: f64,
    tier: u8,
    auxiliary: bool,
    shares_lead: bool,
    reach: usize,
    segments: usize,
    dependents: usize,
}

impl Ranked<'_> {
    fn order(a: &Self, b: &Self) -> Ordering {
        b.tier
            .cmp(&a.tier)
            .then(a.auxiliary.cmp(&b.auxiliary))
            .then(b.shares_lead.cmp(&a.shares_lead))
            .then(b.reach.cmp(&a.reach))
            .then(a.segments.cmp(&b.segments))
            .then(b.dependents.cmp(&a.dependents))
            .then(b.users.total_cmp(&a.users))
            .then(a.candidate.name.len().cmp(&b.candidate.name.len()))
            .then(a.candidate.name.cmp(&b.candidate.name))
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{Spec, World};
    use super::*;

    /// A crate with `users` that depends on the crates with ids `deps`.
    fn member(id: u32, name: &'static str, users: f64, deps: &[u32]) -> Spec {
        let mut spec = Spec::new(id, name);
        spec.users = users;
        spec.deps = deps
            .iter()
            .map(|id| (*id, DependencyKind::Normal))
            .collect();
        spec
    }

    /// The names in rank order for a repository named `repo`.
    fn ranked(repo: &str, specs: Vec<Spec>) -> Vec<String> {
        let world = World::new(specs);
        let members: Vec<Member<'_>> = world
            .candidates
            .iter()
            .map(|c| (c, world.usage.get(&c.crate_id).map_or(0.0, |u| u.current)))
            .collect();
        rank(&world.snapshot, &RepoName::new(repo), &members)
            .iter()
            .map(|c| c.name.clone())
            .collect()
    }

    fn primary(repo: &str, specs: Vec<Spec>) -> String {
        ranked(repo, specs).swap_remove(0)
    }

    /// The shape of the huggingface/candle tree, without its foreign crates.
    fn candle_family() -> Vec<Spec> {
        vec![
            member(1, "candle-kernels", 9.0, &[]),
            member(2, "candle-metal-kernels", 18.0, &[]),
            member(3, "candle-core", 312.0, &[1, 2]),
            member(4, "candle-nn", 266.0, &[1, 2, 3]),
            member(5, "candle-transformers", 222.0, &[3, 4, 6]),
            member(6, "candle-flash-attn", 18.0, &[3]),
            member(7, "candle-onnx", 3.0, &[3, 4]),
            member(8, "candle-datasets", 2.0, &[3, 4]),
            member(9, "candle-pyo3", 0.0, &[3, 4, 7]),
            member(10, "candle-examples", 1.0, &[3, 4, 5, 6, 7, 8]),
        ]
    }

    #[test]
    fn an_examples_crate_that_depends_on_everything_is_not_primary() {
        assert_eq!(primary("candle", candle_family()), "candle-core");
    }

    #[test]
    fn a_kernel_crate_under_the_core_crate_is_not_primary() {
        let found = ranked("candle", candle_family());
        let at = |name: &str| found.iter().position(|n| n == name);
        assert!(at("candle-core") < at("candle-kernels"), "{found:?}");
    }

    #[test]
    fn auxiliary_crates_sort_last() {
        let found = ranked("candle", candle_family());
        assert_eq!(
            found.get(found.len() - 2..).map(<[String]>::to_vec),
            Some(vec!["candle-examples".to_owned(), "candle-pyo3".to_owned()]),
            "{found:?}"
        );
    }

    #[test]
    fn an_auxiliary_crate_wins_when_nothing_else_remains() {
        let found = primary(
            "proj",
            vec![
                member(1, "proj-sys", 1.0, &[]),
                member(2, "proj-cli", 9.0, &[1]),
            ],
        );
        assert_eq!(found, "proj-sys");
    }

    #[test]
    fn the_umbrella_named_like_the_repository_wins_over_every_tree_fact() {
        let found = primary(
            "burn",
            vec![
                member(1, "burn-backend", 2.0, &[]),
                member(2, "burn-core", 7.0, &[1]),
                member(3, "burn", 92.0, &[2]),
            ],
        );
        assert_eq!(found, "burn");
    }

    #[test]
    fn a_hub_named_like_the_repository_wins_over_its_plugins() {
        let found = ranked(
            "proj",
            vec![
                member(1, "proj", 5.0, &[]),
                member(2, "proj-a", 50.0, &[1]),
                member(3, "proj-b", 40.0, &[1]),
            ],
        );
        assert_eq!(found, ["proj", "proj-a", "proj-b"]);
    }

    #[test]
    fn separators_and_language_markers_do_not_hide_the_repository_name() {
        let cli_over_facade = vec![
            member(1, "mistralrs-core", 2.0, &[]),
            member(2, "mistralrs", 18.0, &[1]),
            member(3, "mistralrs-cli", 0.0, &[1, 2]),
        ];
        assert_eq!(primary("mistral.rs", cli_over_facade), "mistralrs");
        let marker = vec![
            member(1, "torch-sys", 3.0, &[]),
            member(2, "tch", 28.0, &[1]),
            member(3, "tch-ext", 0.0, &[1, 2]),
        ];
        assert_eq!(primary("tch-rs", marker), "tch");
        let prefix = vec![member(1, "rust_decimal", 5.0, &[])];
        assert_eq!(primary("rust-decimal", prefix), "rust_decimal");
    }

    #[test]
    fn the_exact_name_wins_over_the_name_without_a_marker() {
        let found = primary(
            "foo-rs",
            vec![member(1, "foo", 50.0, &[]), member(2, "foo-rs", 1.0, &[1])],
        );
        assert_eq!(found, "foo-rs");
    }

    #[test]
    fn a_name_that_only_starts_like_the_repository_is_no_name_match() {
        let found = primary(
            "tract",
            vec![
                member(1, "tractor", 90.0, &[]),
                member(2, "tract-core", 1.0, &[]),
            ],
        );
        assert_eq!(found, "tract-core");
    }

    #[test]
    fn a_crate_sharing_the_repository_word_wins_over_a_crate_with_more_reach() {
        let found = primary(
            "ruvector",
            vec![
                member(1, "prime-radiant", 0.0, &[]),
                member(2, "other-a", 0.0, &[1]),
                member(3, "other-b", 0.0, &[1]),
                member(4, "ruvector-core", 6.0, &[]),
            ],
        );
        assert_eq!(found, "ruvector-core");
    }

    #[test]
    fn the_umbrella_with_the_short_name_wins_a_tie_on_reach() {
        let found = primary(
            "floneum",
            vec![
                member(1, "kalosm-model-types", 0.0, &[]),
                member(2, "kalosm", 1.0, &[1]),
            ],
        );
        assert_eq!(found, "kalosm");
        let found = primary(
            "kreuzberg-lts",
            vec![
                member(1, "kreuzberg-paddle-ocr", 0.0, &[]),
                member(2, "kreuzberg", 3.0, &[1]),
            ],
        );
        assert_eq!(found, "kreuzberg");
    }

    #[test]
    fn the_crate_a_family_builds_on_wins_over_a_crate_on_top_of_it() {
        let found = ranked(
            "proj",
            vec![
                member(1, "proj-core", 5.0, &[]),
                member(2, "proj-nn", 20.0, &[1]),
                member(3, "proj-api", 90.0, &[1, 2]),
            ],
        );
        assert_eq!(found, ["proj-core", "proj-nn", "proj-api"]);
    }

    #[test]
    fn users_do_not_outrank_the_tree() {
        let found = primary(
            "proj",
            vec![
                member(1, "proj-base", 1.0, &[]),
                member(2, "proj-leaf", 999.0, &[1]),
                member(3, "proj-tool", 999.0, &[1]),
            ],
        );
        assert_eq!(found, "proj-base");
    }

    #[test]
    fn crates_outside_the_tree_tie_on_users_then_shortest_then_alphabetical_name() {
        let found = ranked(
            "repo",
            vec![
                member(1, "x-bb", 5.0, &[]),
                member(2, "x-aa", 5.0, &[]),
                member(3, "x-c", 5.0, &[]),
                member(4, "x-top", 9.0, &[]),
            ],
        );
        assert_eq!(found, ["x-top", "x-c", "x-aa", "x-bb"]);
    }

    #[test]
    fn a_cycle_ends() {
        let found = ranked(
            "repo",
            vec![member(1, "x-a", 1.0, &[2]), member(2, "x-b", 9.0, &[1])],
        );
        assert_eq!(found, ["x-b", "x-a"]);
    }

    #[test]
    fn dev_and_outside_dependencies_are_ignored() {
        let mut dev = member(1, "x-a", 1.0, &[]);
        dev.deps = vec![(2, DependencyKind::Dev), (99, DependencyKind::Normal)];
        let found = ranked("repo", vec![dev, member(2, "x-b", 9.0, &[])]);
        assert_eq!(found, ["x-b", "x-a"]);
    }

    #[test]
    fn a_build_dependency_counts() {
        let mut build = member(1, "x-a", 9.0, &[]);
        build.deps = vec![(2, DependencyKind::Build)];
        let found = ranked("repo", vec![build, member(2, "x-b", 1.0, &[])]);
        assert_eq!(found, ["x-b", "x-a"]);
    }

    #[test]
    fn a_rust_named_crate_relates_to_the_repository_or_carries_a_marker() {
        let repo = RepoName::new("some-engine");
        for name in [
            "some-engine",
            "some_engine",
            "some-engine-sys",
            "some",
            "rust-other",
            "other-rs",
        ] {
            assert!(repo.is_rust_named(name), "{name}");
        }
        for name in ["other", "other-engine"] {
            assert!(!repo.is_rust_named(name), "{name}");
        }
    }

    #[test]
    fn names_repo_compares_words_without_filler() {
        let yes = [
            ("oxen", "liboxen"),
            ("foo-rs", "foo"),
            ("mistral.rs", "mistralrs"),
            ("mistral.rs", "mistralrs-core"),
            ("some-engine", "other_engine"),
            ("meilisearch", "meilisearch-sdk"),
        ];
        for (repo, name) in yes {
            assert!(RepoName::new(repo).names_repo(name), "{repo} {name}");
        }
        let no = [
            ("engine", "unrelated"),
            ("meilisearch", "interprete"),
            ("foo-rs", "rust-lib"),
        ];
        for (repo, name) in no {
            assert!(!RepoName::new(repo).names_repo(name), "{repo} {name}");
        }
    }

    #[test]
    fn lead_word_skips_a_language_marker() {
        assert_eq!(lead_word("candle-core").as_deref(), Some("candle"));
        assert_eq!(lead_word("rust_stemmers").as_deref(), Some("stemmers"));
        assert_eq!(lead_word("rust").as_deref(), Some("rust"));
    }
}
