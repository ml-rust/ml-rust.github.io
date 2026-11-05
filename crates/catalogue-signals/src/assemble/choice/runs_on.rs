//! What a project runs on, from the dependency and feature names of its listed crates.
//!
//! The check reads the default version of each listed crate: its direct dependencies and its
//! feature names. A GPU backend behind a deeper dependency stays unseen. So does a backend that
//! needs no feature. A value with no match states only that nothing matched.

use catalogue_core::{Backend, Backends, NO_GPU_FEATURE, RUNS_ON_SEPARATOR, canonical_crate_name};

use super::family::Member;
use super::reach::enabling_feature;
use crate::dump::DumpSnapshot;

/// The text between the backends in the evidence value.
const EVIDENCE_SEPARATOR: &str = "; ";

/// The evidence value of a project with no backend row.
const NO_ROW: &str = "no dependency or feature matches a backend row";

/// What the listed crates of one entry show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunsOn {
    /// The backends with a hit, in table order.
    Backends(Vec<BackendHits>),
    /// The dump holds the crates and no row matches.
    NoGpuFeature,
}

/// One backend and what shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendHits {
    /// The backend name.
    pub name: String,
    /// One hit per crate that shows it, in entry order.
    pub hits: Vec<Hit>,
}

/// One listed crate that shows a backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The listed crate.
    pub crate_name: String,
    /// What shows the backend.
    pub shows: Shows,
}

/// What on a listed crate shows a backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shows {
    /// The table names the listed crate itself.
    Crate,
    /// A dependency that is always on.
    Dependency(String),
    /// A feature turns the backend on.
    Feature {
        /// The feature name.
        feature: String,
        /// The optional dependency it enables. `None` when the feature name alone shows it.
        dependency: Option<String>,
    },
}

impl RunsOn {
    /// The value of the `runs_on` fact: the backend names, or [`NO_GPU_FEATURE`].
    pub fn value(&self) -> String {
        match self {
            Self::NoGpuFeature => NO_GPU_FEATURE.to_owned(),
            Self::Backends(found) => {
                let names: Vec<&str> = found.iter().map(|hit| hit.name.as_str()).collect();
                names.join(RUNS_ON_SEPARATOR)
            }
        }
    }

    /// The value of the `runs_on_evidence` fact: the crate and the name behind each backend.
    pub fn evidence(&self) -> String {
        match self {
            Self::NoGpuFeature => NO_ROW.to_owned(),
            Self::Backends(found) => {
                let lines: Vec<String> = found.iter().map(BackendHits::evidence).collect();
                lines.join(EVIDENCE_SEPARATOR)
            }
        }
    }
}

impl BackendHits {
    fn evidence(&self) -> String {
        let hits: Vec<String> = self.hits.iter().map(Hit::evidence).collect();
        format!("{}: {}", self.name, hits.join(RUNS_ON_SEPARATOR))
    }
}

impl Hit {
    fn evidence(&self) -> String {
        let name = &self.crate_name;
        match &self.shows {
            Shows::Crate => format!("{name} itself"),
            Shows::Dependency(dependency) => format!("{name} dependency {dependency}"),
            Shows::Feature {
                feature,
                dependency: Some(dependency),
            } => format!("{name} feature {feature} (dependency {dependency})"),
            Shows::Feature {
                feature,
                dependency: None,
            } => format!("{name} feature {feature}"),
        }
    }
}

/// What `members` show under `table`.
///
/// `None` without a member or without a backend row: the check measured nothing.
pub(super) fn runs_on(
    members: &[Member<'_>],
    dump: &DumpSnapshot,
    table: &Backends,
) -> Option<RunsOn> {
    if members.is_empty() || table.backend.is_empty() {
        return None;
    }
    let found: Vec<BackendHits> = table
        .backend
        .iter()
        .filter_map(|backend| {
            let hits: Vec<Hit> = members
                .iter()
                .filter_map(|member| hit_of(member, dump, backend))
                .collect();
            (!hits.is_empty()).then(|| BackendHits {
                name: backend.name.clone(),
                hits,
            })
        })
        .collect();
    Some(if found.is_empty() {
        RunsOn::NoGpuFeature
    } else {
        RunsOn::Backends(found)
    })
}

/// What on `member` shows `backend`.
///
/// The crate itself comes first, then a dependency that is always on, then an optional
/// dependency with the feature that enables it, then a feature name alone.
fn hit_of(member: &Member<'_>, dump: &DumpSnapshot, backend: &Backend) -> Option<Hit> {
    let listed = |name: &str| {
        let canonical = canonical_crate_name(name);
        backend
            .dependencies
            .iter()
            .any(|dependency| dependency.normalized() == canonical)
    };
    let shows = if listed(member.name) {
        Shows::Crate
    } else {
        let matched: Vec<_> = member
            .dependencies(dump)
            .filter(|dependency| listed(dependency.name))
            .collect();
        if let Some(always) = matched.iter().find(|dependency| !dependency.optional) {
            Shows::Dependency(always.name.to_owned())
        } else if let Some(optional) = matched.first() {
            Shows::Feature {
                feature: enabling_feature(member, optional, &backend.features),
                dependency: Some(optional.name.to_owned()),
            }
        } else {
            let feature = member.details.features.keys().find(|feature| {
                backend
                    .features
                    .iter()
                    .any(|name| feature.eq_ignore_ascii_case(name))
            })?;
            Shows::Feature {
                feature: feature.clone(),
                dependency: None,
            }
        }
    };
    Some(Hit {
        crate_name: member.name.to_owned(),
        shows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::choice::family::members;
    use crate::assemble::fixtures::{Dump, library};
    use crate::assemble::index::CrateIndex;
    use crate::dump::{CrateDetails, DependencyKind, Rename};

    const TABLE: &str = r#"
[[backend]]
id = "cuda"
name = "CUDA"
dependencies = ["cudarc"]
features = ["cuda"]

[[backend]]
id = "metal"
name = "Metal"
dependencies = ["objc2-metal"]
features = ["metal"]
"#;

    fn table() -> Backends {
        toml::from_str(TABLE).expect("table parses")
    }

    fn features(items: &[(&str, &[&str])]) -> CrateDetails {
        CrateDetails {
            features: items
                .iter()
                .map(|(name, list)| {
                    let list = list.iter().map(|item| (*item).to_owned()).collect();
                    ((*name).to_owned(), list)
                })
                .collect(),
            ..CrateDetails::default()
        }
    }

    /// A dump with `tool` as crate 1 and the crates the requirements name.
    fn dump() -> Dump {
        let mut dump = Dump::new();
        dump.add_crate(1, "tool");
        dump.add_crate(2, "tool-extra");
        dump.add_crate(10, "cudarc");
        dump.add_crate(11, "objc2-metal");
        dump
    }

    fn measure(dump: &Dump, crates: &[&str], table: &Backends) -> Option<RunsOn> {
        let index = CrateIndex::new(&dump.snapshot);
        let entry = library(crates);
        let found = members(&entry, &dump.snapshot, &index);
        runs_on(&found, &dump.snapshot, table)
    }

    #[test]
    fn a_required_dependency_shows_the_backend_with_no_feature() {
        let mut dump = dump();
        dump.set_details(1, CrateDetails::default());
        dump.add_requirement(1, 10, "^1", DependencyKind::Normal);
        let found = measure(&dump, &["tool"], &table()).expect("measured");
        assert_eq!(found.value(), "CUDA");
        assert_eq!(found.evidence(), "CUDA: tool dependency cudarc");
    }

    #[test]
    fn a_feature_only_dependency_records_the_feature_that_reaches_it() {
        let mut dump = dump();
        dump.set_details(
            1,
            features(&[("gpu", &["dep:cudarc"]), ("nccl", &["gpu", "cudarc/nccl"])]),
        );
        dump.add_optional(1, 10);
        let found = measure(&dump, &["tool"], &table()).expect("measured");
        assert_eq!(found.value(), "CUDA");
        assert_eq!(
            found.evidence(),
            "CUDA: tool feature gpu (dependency cudarc)"
        );
    }

    #[test]
    fn a_renamed_optional_dependency_is_reached_through_its_new_name() {
        let mut dump = dump();
        let mut details = features(&[("accel", &["dep:gpu-api"])]);
        details.renames.push(Rename {
            crate_id: 10,
            name: "gpu-api".to_owned(),
        });
        dump.set_details(1, details);
        dump.add_optional(1, 10);
        let found = measure(&dump, &["tool"], &table()).expect("measured");
        assert_eq!(
            found.evidence(),
            "CUDA: tool feature accel (dependency cudarc)"
        );
    }

    #[test]
    fn a_feature_name_alone_shows_the_backend() {
        let mut dump = dump();
        dump.set_details(
            1,
            features(&[("metal", &["tool-sys/metal"]), ("cuda-tests", &[])]),
        );
        let found = measure(&dump, &["tool"], &table()).expect("measured");
        assert_eq!(found.value(), "Metal", "a feature name matches whole");
        assert_eq!(found.evidence(), "Metal: tool feature metal");
    }

    #[test]
    fn a_dev_or_build_dependency_shows_nothing() {
        let mut dump = dump();
        dump.set_details(1, CrateDetails::default());
        dump.add_requirement(1, 10, "^1", DependencyKind::Dev);
        dump.add_requirement(1, 11, "^1", DependencyKind::Build);
        let found = measure(&dump, &["tool"], &table()).expect("measured");
        assert_eq!(found, RunsOn::NoGpuFeature);
        assert_eq!(found.value(), "GPU support unknown");
    }

    #[test]
    fn a_listed_crate_the_table_names_shows_the_backend() {
        let mut dump = dump();
        dump.set_details(10, CrateDetails::default());
        let found = measure(&dump, &["cudarc"], &table()).expect("measured");
        assert_eq!(found.value(), "CUDA");
        assert_eq!(found.evidence(), "CUDA: cudarc itself");
    }

    #[test]
    fn every_listed_crate_counts_and_backends_keep_the_table_order() {
        let mut dump = dump();
        dump.set_details(1, features(&[("metal", &[])]));
        dump.set_details(2, CrateDetails::default());
        dump.add_requirement(2, 10, "^1", DependencyKind::Normal);
        dump.add_requirement(1, 10, "^1", DependencyKind::Normal);
        let found = measure(&dump, &["tool", "tool-extra"], &table()).expect("measured");
        assert_eq!(found.value(), "CUDA, Metal");
        assert_eq!(
            found.evidence(),
            "CUDA: tool dependency cudarc, tool-extra dependency cudarc; Metal: tool feature metal"
        );
    }

    #[test]
    fn an_empty_table_measures_nothing() {
        let mut dump = dump();
        dump.set_details(1, features(&[("cuda", &[])]));
        assert_eq!(measure(&dump, &["tool"], &Backends::default()), None);
    }

    #[test]
    fn a_crate_with_no_details_measures_nothing() {
        assert_eq!(measure(&dump(), &["tool"], &table()), None);
        assert_eq!(measure(&dump(), &["absent"], &table()), None);
    }
}
