//! Which feature turns on an optional dependency.

use super::family::{Dependency, Member};

/// The feature that default features name for themselves.
const DEFAULT: &str = "default";

/// The feature of `member` that enables the optional dependency `dependency`.
///
/// A feature enables it when its list holds `name`, `dep:name` or `name/feature`, under the crate
/// name or a rename. The weak form `name?/feature` enables nothing. Among several, a feature in
/// `preferred` comes first, then the first by name, with `default` last. With none, Cargo makes
/// an implicit feature named after the dependency, so that name is the answer.
pub(super) fn enabling_feature(
    member: &Member<'_>,
    dependency: &Dependency<'_>,
    preferred: &[String],
) -> String {
    let renames = member
        .details
        .renames
        .iter()
        .filter(|rename| rename.crate_id == dependency.crate_id)
        .map(|rename| rename.name.as_str());
    let names: Vec<&str> = std::iter::once(dependency.name).chain(renames).collect();
    let enables = |item: &str| {
        let head = item.split_once('/').map_or(item, |(head, _)| head);
        let head = head.strip_prefix("dep:").unwrap_or(head);
        names.contains(&head)
    };
    let found: Vec<&str> = member
        .details
        .features
        .iter()
        .filter(|(_, items)| items.iter().any(|item| enables(item)))
        .map(|(feature, _)| feature.as_str())
        .collect();
    let chosen = found
        .iter()
        .find(|feature| {
            preferred
                .iter()
                .any(|name| feature.eq_ignore_ascii_case(name))
        })
        .or_else(|| found.iter().find(|feature| **feature != DEFAULT))
        .or_else(|| found.first());
    let implicit = names.last().copied().unwrap_or(dependency.name);
    chosen.copied().unwrap_or(implicit).to_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::dump::{CrateDetails, Rename};

    fn details(features: &[(&str, &[&str])], renames: Vec<Rename>) -> CrateDetails {
        CrateDetails {
            license: None,
            links: None,
            features: features
                .iter()
                .map(|(name, items)| {
                    let items = items.iter().map(|item| (*item).to_owned()).collect();
                    ((*name).to_owned(), items)
                })
                .collect::<BTreeMap<_, _>>(),
            renames,
        }
    }

    fn reach(found: &CrateDetails, preferred: &[&str]) -> String {
        let member = Member {
            name: "tool",
            details: found,
            requirements: &[],
        };
        let dependency = Dependency {
            crate_id: 9,
            name: "cudarc",
            optional: true,
        };
        let preferred: Vec<String> = preferred.iter().map(|name| (*name).to_owned()).collect();
        enabling_feature(&member, &dependency, &preferred)
    }

    #[test]
    fn each_strong_form_enables_the_dependency() {
        for item in ["dep:cudarc", "cudarc", "cudarc/nccl"] {
            let found = details(&[("gpu", &[item])], Vec::new());
            assert_eq!(reach(&found, &[]), "gpu", "{item}");
        }
    }

    #[test]
    fn a_weak_form_enables_nothing() {
        let found = details(&[("gpu", &["cudarc?/nccl"])], Vec::new());
        assert_eq!(reach(&found, &[]), "cudarc", "the implicit feature");
    }

    #[test]
    fn a_preferred_feature_wins_and_default_comes_last() {
        let found = details(
            &[
                ("all", &["dep:cudarc"]),
                ("cuda", &["dep:cudarc"]),
                ("default", &["dep:cudarc"]),
            ],
            Vec::new(),
        );
        assert_eq!(reach(&found, &["cuda"]), "cuda");
        assert_eq!(reach(&found, &[]), "all");
        let only_default = details(&[("default", &["dep:cudarc"])], Vec::new());
        assert_eq!(reach(&only_default, &[]), "default");
    }

    #[test]
    fn a_rename_counts_and_names_the_implicit_feature() {
        let rename = || {
            vec![Rename {
                crate_id: 9,
                name: "gpu-api".to_owned(),
            }]
        };
        let listed = details(&[("gpu", &["dep:gpu-api"])], rename());
        assert_eq!(reach(&listed, &[]), "gpu");
        let implicit = details(&[], rename());
        assert_eq!(reach(&implicit, &[]), "gpu-api");
    }
}
