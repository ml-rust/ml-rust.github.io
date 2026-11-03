//! The selection of candidate crates.

use std::borrow::Cow;
use std::collections::HashSet;

use super::core_crates;
use super::types::{Candidate, Discovered, Reason, Rules};
use crate::dump::{DependencyKind, DumpSnapshot, IdMap};

/// Selects the crates of `snapshot` that match `rules`.
///
/// A crate matches when it is a core crate, when one of its category slugs
/// equals or extends a rule category, when one of its keywords equals a rule
/// keyword, or when its default version has a normal or build dependency on a
/// core crate. A core crate in the snapshot is a candidate with the reason
/// `core` even when no other rule matches it, and keeps its other reasons. A crate
/// whose default version is missing or yanked is skipped. A crate with a
/// deprecation notice stays and is flagged. The run reads the category, keyword
/// and default-requirement tables once each and never reads the dependency edges.
pub fn candidates(snapshot: &DumpSnapshot, rules: &Rules) -> Discovered {
    let core = core_crates::resolve(snapshot, &rules.core_crates);
    let mut reasons: IdMap<Vec<Reason>> = IdMap::default();
    for &id in core.by_id.keys() {
        reasons.entry(id).or_default().push(Reason::Core);
    }

    let categories: HashSet<&str> = rules.categories.iter().map(String::as_str).collect();
    for (&id, slugs) in &snapshot.categories {
        for slug in slugs.iter().filter(|slug| in_category(&categories, slug)) {
            reasons
                .entry(id)
                .or_default()
                .push(Reason::Category(slug.clone()));
        }
    }

    let keywords: HashSet<String> = rules.keywords.iter().map(|k| k.to_lowercase()).collect();
    for (&id, found) in &snapshot.keywords {
        for keyword in found {
            let lower = lowercase(keyword);
            if keywords.contains(&*lower) {
                reasons
                    .entry(id)
                    .or_default()
                    .push(Reason::Keyword(lower.into_owned()));
            }
        }
    }

    for (&id, requirements) in &snapshot.default_requirements {
        for requirement in requirements {
            let counts = matches!(
                requirement.kind,
                DependencyKind::Normal | DependencyKind::Build
            );
            if let Some(name) = counts
                .then(|| core.by_id.get(&requirement.crate_id))
                .flatten()
            {
                reasons
                    .entry(id)
                    .or_default()
                    .push(Reason::DependsOn(name.clone()));
            }
        }
    }

    let mut found: Vec<Candidate> = reasons
        .into_iter()
        .filter_map(|(id, reasons)| candidate(snapshot, id, reasons))
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name).then(a.crate_id.cmp(&b.crate_id)));
    Discovered {
        candidates: found,
        unknown_core_crates: core.unknown,
    }
}

/// Builds the candidate for `id`, or `None` when the crate or its unyanked default version is absent.
fn candidate(snapshot: &DumpSnapshot, id: u32, mut reasons: Vec<Reason>) -> Option<Candidate> {
    let row = snapshot.crates.get(&id)?;
    let default = snapshot
        .default_version
        .get(&id)
        .and_then(|version| snapshot.versions.get(version))?;
    if default.yanked {
        return None;
    }
    reasons.sort();
    reasons.dedup();
    Some(Candidate {
        crate_id: id,
        name: row.name.clone(),
        reasons,
        deprecated: row.deprecation.is_some(),
    })
}

/// Whether `slug`, or a parent made of leading `::` segments, is a rule category.
fn in_category(rules: &HashSet<&str>, slug: &str) -> bool {
    rules.contains(slug)
        || slug
            .match_indices("::")
            .any(|(end, _)| slug.get(..end).is_some_and(|parent| rules.contains(parent)))
}

fn lowercase(text: &str) -> Cow<'_, str> {
    if text.chars().any(char::is_uppercase) {
        Cow::Owned(text.to_lowercase())
    } else {
        Cow::Borrowed(text)
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::deprecation::{DeprecationNotice, Field};
    use jiff::Timestamp;

    use super::*;
    use crate::dump::{CrateRow, DumpMeta, Requirement, VersionRow};

    const ML: u32 = 1;
    const APP: u32 = 2;
    const CORE: u32 = 3;
    const OTHER: u32 = 4;

    fn rules(categories: &[&str], keywords: &[&str], core: &[&str]) -> Rules {
        let own = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect();
        Rules {
            categories: own(categories),
            keywords: own(keywords),
            core_crates: own(core),
        }
    }

    /// Crates `ML` (id 1) to `OTHER` (id 4), each with an unyanked default version.
    fn snapshot() -> DumpSnapshot {
        let mut snap = DumpSnapshot {
            meta: DumpMeta {
                timestamp: Timestamp::UNIX_EPOCH,
                crates_io_commit: String::new(),
                directory: String::new(),
            },
            crates: Default::default(),
            versions: Default::default(),
            default_version: Default::default(),
            edges: Vec::new(),
            default_requirements: Default::default(),
            owners: Default::default(),
            users: Default::default(),
            teams: Default::default(),
            categories: Default::default(),
            keywords: Default::default(),
            deleted: Vec::new(),
            downloads: Default::default(),
            warnings: Default::default(),
        };
        let crates = [
            (ML, "ml-thing"),
            (APP, "app"),
            (CORE, "Candle_Core"),
            (OTHER, "plain"),
        ];
        for (id, name) in crates {
            add_crate(&mut snap, id, name, false);
        }
        snap
    }

    fn add_crate(snap: &mut DumpSnapshot, id: u32, name: &str, yanked: bool) {
        snap.crates.insert(
            id,
            CrateRow {
                name: name.to_owned(),
                created_at: Timestamp::UNIX_EPOCH,
                updated_at: Timestamp::UNIX_EPOCH,
                description: None,
                homepage: None,
                repository: None,
                deprecation: None,
                has_readme: false,
            },
        );
        snap.versions.insert(
            id,
            VersionRow {
                crate_id: id,
                num: "1.0.0".to_owned(),
                created_at: Timestamp::UNIX_EPOCH,
                yanked,
                rust_version: None,
                edition: None,
                published_by: None,
            },
        );
        snap.default_version.insert(id, id);
    }

    fn requirement(crate_id: u32, kind: DependencyKind) -> Requirement {
        Requirement {
            req: "^1".to_owned(),
            crate_id,
            kind,
            optional: false,
        }
    }

    fn names(found: &Discovered) -> Vec<&str> {
        found.candidates.iter().map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn category_matches_exactly() {
        let mut snap = snapshot();
        snap.categories.insert(ML, vec!["science::ml".to_owned()]);
        snap.categories.insert(OTHER, vec!["science".to_owned()]);
        let found = candidates(&snap, &rules(&["science::ml"], &[], &[]));
        assert_eq!(names(&found), ["ml-thing"]);
        assert_eq!(
            found.candidates[0].reasons,
            [Reason::Category("science::ml".to_owned())]
        );
    }

    #[test]
    fn category_matches_sub_categories_only_at_a_segment_boundary() {
        let mut snap = snapshot();
        snap.categories
            .insert(ML, vec!["science::ml::vision".to_owned()]);
        snap.categories.insert(APP, vec!["science::mlx".to_owned()]);
        let found = candidates(&snap, &rules(&["science::ml"], &[], &[]));
        assert_eq!(names(&found), ["ml-thing"]);
        assert_eq!(
            found.candidates[0].reasons,
            [Reason::Category("science::ml::vision".to_owned())]
        );
    }

    #[test]
    fn keyword_matches_exactly_and_without_case() {
        let mut snap = snapshot();
        snap.keywords.insert(ML, vec!["LLM".to_owned()]);
        snap.keywords.insert(APP, vec!["llm-tools".to_owned()]);
        let found = candidates(&snap, &rules(&[], &["llm"], &[]));
        assert_eq!(names(&found), ["ml-thing"]);
        assert_eq!(
            found.candidates[0].reasons,
            [Reason::Keyword("llm".to_owned())]
        );
        let found = candidates(&snap, &rules(&[], &["LLM"], &[]));
        assert_eq!(names(&found), ["ml-thing"]);
    }

    #[test]
    fn normal_and_build_dependencies_on_a_core_crate_match() {
        let mut snap = snapshot();
        snap.default_requirements
            .insert(ML, vec![requirement(CORE, DependencyKind::Normal)]);
        snap.default_requirements
            .insert(APP, vec![requirement(CORE, DependencyKind::Build)]);
        let found = candidates(&snap, &rules(&[], &[], &["candle-core"]));
        assert_eq!(names(&found), ["Candle_Core", "app", "ml-thing"]);
        assert_eq!(
            found.candidates[1].reasons,
            [Reason::DependsOn("candle-core".to_owned())]
        );
    }

    #[test]
    fn dev_dependencies_do_not_match() {
        let mut snap = snapshot();
        snap.default_requirements
            .insert(ML, vec![requirement(CORE, DependencyKind::Dev)]);
        let found = candidates(&snap, &rules(&[], &[], &["candle-core"]));
        assert_eq!(names(&found), ["Candle_Core"]);
        assert!(found.unknown_core_crates.is_empty());
    }

    #[test]
    fn core_names_resolve_the_crates_io_way() {
        let mut snap = snapshot();
        snap.default_requirements
            .insert(ML, vec![requirement(CORE, DependencyKind::Normal)]);
        for spelling in ["candle-core", "CANDLE_CORE", "Candle_Core"] {
            let found = candidates(&snap, &rules(&[], &[], &[spelling]));
            assert_eq!(names(&found), ["Candle_Core", "ml-thing"], "{spelling}");
            assert_eq!(
                found.candidates[1].reasons,
                [Reason::DependsOn(spelling.to_owned())]
            );
            assert!(found.unknown_core_crates.is_empty());
        }
    }

    #[test]
    fn unknown_core_crates_are_reported_in_rule_order_without_repeats() {
        let snap = snapshot();
        let found = candidates(
            &snap,
            &rules(&[], &[], &["zeta", "candle-core", "Alpha", "ZETA"]),
        );
        assert_eq!(found.unknown_core_crates, ["zeta", "Alpha"]);
        assert_eq!(names(&found), ["Candle_Core"]);
    }

    #[test]
    fn a_core_crate_without_another_match_is_a_candidate() {
        let snap = snapshot();
        let found = candidates(&snap, &rules(&[], &[], &["candle-core"]));
        assert_eq!(names(&found), ["Candle_Core"]);
        assert_eq!(found.candidates[0].reasons, [Reason::Core]);
    }

    #[test]
    fn a_core_crate_keeps_its_other_reasons() {
        let mut snap = snapshot();
        snap.keywords.insert(CORE, vec!["tensor".to_owned()]);
        let found = candidates(&snap, &rules(&[], &["tensor"], &["candle-core"]));
        assert_eq!(
            found.candidates[0].reasons,
            [Reason::Core, Reason::Keyword("tensor".to_owned())]
        );
    }

    #[test]
    fn a_yanked_core_crate_is_skipped() {
        let mut snap = snapshot();
        add_crate(&mut snap, CORE, "Candle_Core", true);
        let found = candidates(&snap, &rules(&[], &[], &["candle-core"]));
        assert!(found.candidates.is_empty());
    }

    #[test]
    fn core_crates_match_rules_like_any_crate() {
        let mut snap = snapshot();
        snap.keywords.insert(CORE, vec!["tensor".to_owned()]);
        let found = candidates(&snap, &rules(&[], &["tensor"], &["candle-core"]));
        assert_eq!(names(&found), ["Candle_Core"]);
    }

    #[test]
    fn crate_without_a_match_is_not_a_candidate() {
        let snap = snapshot();
        let found = candidates(&snap, &rules(&["science::ml"], &["llm"], &["missing"]));
        assert!(found.candidates.is_empty());
    }

    #[test]
    fn yanked_default_version_is_skipped() {
        let mut snap = snapshot();
        add_crate(&mut snap, ML, "ml-thing", true);
        snap.keywords.insert(ML, vec!["llm".to_owned()]);
        snap.keywords.insert(APP, vec!["llm".to_owned()]);
        let found = candidates(&snap, &rules(&[], &["llm"], &[]));
        assert_eq!(names(&found), ["app"]);
    }

    #[test]
    fn missing_default_version_is_skipped() {
        let mut snap = snapshot();
        snap.default_version.remove(&ML);
        snap.keywords.insert(ML, vec!["llm".to_owned()]);
        let found = candidates(&snap, &rules(&[], &["llm"], &[]));
        assert!(found.candidates.is_empty());
    }

    #[test]
    fn deprecated_crate_is_kept_and_flagged() {
        let mut snap = snapshot();
        snap.keywords.insert(ML, vec!["llm".to_owned()]);
        snap.keywords.insert(APP, vec!["llm".to_owned()]);
        if let Some(row) = snap.crates.get_mut(&ML) {
            row.deprecation = Some(DeprecationNotice {
                field: Field::Description,
                phrase: "Deprecated".to_owned(),
            });
        }
        let found = candidates(&snap, &rules(&[], &["llm"], &[]));
        let flags: Vec<(&str, bool)> = found
            .candidates
            .iter()
            .map(|c| (c.name.as_str(), c.deprecated))
            .collect();
        assert_eq!(flags, [("app", false), ("ml-thing", true)]);
    }

    #[test]
    fn reasons_are_sorted_and_deduplicated() {
        let mut snap = snapshot();
        snap.categories.insert(ML, vec!["science::ml".to_owned()]);
        snap.keywords.insert(
            ML,
            vec!["tensor".to_owned(), "llm".to_owned(), "llm".to_owned()],
        );
        snap.default_requirements.insert(
            ML,
            vec![
                requirement(CORE, DependencyKind::Normal),
                requirement(CORE, DependencyKind::Build),
            ],
        );
        let found = candidates(
            &snap,
            &rules(&["science"], &["llm", "tensor"], &["candle-core"]),
        );
        assert_eq!(names(&found), ["Candle_Core", "ml-thing"]);
        assert_eq!(
            found.candidates[1].reasons,
            [
                Reason::Category("science::ml".to_owned()),
                Reason::Keyword("llm".to_owned()),
                Reason::Keyword("tensor".to_owned()),
                Reason::DependsOn("candle-core".to_owned()),
            ]
        );
    }

    #[test]
    fn candidates_are_sorted_by_name() {
        let mut snap = snapshot();
        for id in [ML, APP, OTHER] {
            snap.keywords.insert(id, vec!["llm".to_owned()]);
        }
        let found = candidates(&snap, &rules(&[], &["llm"], &[]));
        assert_eq!(names(&found), ["app", "ml-thing", "plain"]);
    }

    #[test]
    fn reasons_display_with_their_prefix() {
        assert_eq!(Reason::Core.to_string(), "core");
        assert_eq!(
            Reason::Category("a::b".to_owned()).to_string(),
            "category:a::b"
        );
        assert_eq!(Reason::Keyword("llm".to_owned()).to_string(), "keyword:llm");
        assert_eq!(
            Reason::DependsOn("burn".to_owned()).to_string(),
            "depends-on:burn"
        );
    }
}
