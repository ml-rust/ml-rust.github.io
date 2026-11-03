use std::collections::{BTreeMap, BTreeSet};

use super::super::diagnostics::Diagnostics;
use super::supersedes::Link;
use super::{Refs, approval, references, supersedes};
use crate::catalogue::Catalogue;
use crate::ids::{GithubLogin, Slug};
use crate::judgment::{Judgment, JudgmentKind};

/// The messages of `diags`, without their paths.
fn messages(diags: &Diagnostics) -> Vec<String> {
    diags.iter().map(|d| d.message.clone()).collect()
}

/// Runs `f` with the references that `catalogue` holds.
fn with_refs<T>(catalogue: &Catalogue, f: impl FnOnce(&Refs<'_>) -> T) -> T {
    let topics: BTreeSet<Slug> = catalogue.topics.iter().map(|t| t.id.clone()).collect();
    let mut maintainers: BTreeMap<String, bool> = BTreeMap::new();
    for maintainer in &catalogue.maintainers {
        maintainers
            .entry(maintainer.login.normalized())
            .or_insert(maintainer.org_member);
    }
    f(&Refs {
        entries: &catalogue.entries,
        topics: &topics,
        maintainers: &maintainers,
    })
}

/// Lists the reference rules that `judgment` breaks against `catalogue`.
///
/// Covers the entry, the replacing entry, the topics and a replacement cycle.
pub(crate) fn reference_problems(catalogue: &Catalogue, judgment: &Judgment) -> Vec<String> {
    let mut diags = Diagnostics::new();
    with_refs(catalogue, |refs| {
        references::check("", judgment, refs, &mut diags);
    });
    let mut links: Vec<Link> = catalogue
        .judgments
        .values()
        .filter_map(|existing| Link::of("", existing))
        .collect();
    links.extend(Link::of("", judgment));
    supersedes::check(&links, &mut diags);
    messages(&diags)
}

/// Lists the approval rules that `judgment` breaks against `catalogue`.
pub(crate) fn approval_problems(catalogue: &Catalogue, judgment: &Judgment) -> Vec<String> {
    let mut diags = Diagnostics::new();
    with_refs(catalogue, |refs| {
        approval::check("", judgment, refs, &mut diags);
    });
    messages(&diags)
}

/// The maintainers who break no approval rule for `kind` on `entry` from `reporter`.
pub(crate) fn eligible_approvers(
    catalogue: &Catalogue,
    entry: &Slug,
    reporter: &GithubLogin,
    kind: &JudgmentKind,
) -> Vec<GithubLogin> {
    with_refs(catalogue, |refs| {
        catalogue
            .maintainers
            .iter()
            .filter(|m| approval::problems(entry, kind, reporter, &m.login, refs).is_empty())
            .map(|m| m.login.clone())
            .collect()
    })
}
