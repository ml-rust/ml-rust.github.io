//! Hand-built dumps and topics for the drafting tests.

use catalogue_core::{Date, Topics};
use jiff::Timestamp;

use crate::dump::{
    CrateRow, DependencyKind, DumpMeta, DumpSnapshot, IdMap, Owner, OwnerKind, Requirement,
    VersionRow,
};
use crate::usage::Usage;

/// A user owner.
pub(super) fn user(id: u32) -> Owner {
    Owner {
        id,
        kind: OwnerKind::User,
    }
}

/// A team owner.
pub(super) fn team(id: u32) -> Owner {
    Owner {
        id,
        kind: OwnerKind::Team,
    }
}

/// The draft date of every test.
pub(super) fn today() -> Date {
    Date::new(2026, 11, 3).expect("date")
}

/// The topics of every test.
pub(super) fn topics() -> Topics {
    toml::from_str(
        r#"
[[topic]]
id = "nlp"
name = "NLP"
keywords = ["nlp"]

[[topic]]
id = "foundations"
name = "Foundations"
keywords = ["tensor"]
categories = ["science::math"]
terms = ["sparse matrix"]
"#,
    )
    .expect("topics parse")
}

/// A crate that passes every rule, until a test changes a field.
pub(super) struct Spec {
    pub(super) id: u32,
    pub(super) name: &'static str,
    pub(super) description: Option<String>,
    pub(super) repository: Option<String>,
    pub(super) homepage: Option<&'static str>,
    pub(super) keywords: Vec<&'static str>,
    pub(super) categories: Vec<&'static str>,
    pub(super) yanked: bool,
    pub(super) deprecated: bool,
    pub(super) users: f64,
    /// The crate ids its default version depends on, with the kind.
    pub(super) deps: Vec<(u32, DependencyKind)>,
    pub(super) owners: Vec<Owner>,
}

impl Spec {
    /// A tokenizer crate with 5 users, one owner and its own GitHub repository.
    pub(super) fn new(id: u32, name: &'static str) -> Self {
        Self {
            id,
            name,
            description: Some("A tokenizer.".to_owned()),
            repository: Some(format!("https://github.com/someone/{name}")),
            homepage: None,
            keywords: vec!["nlp"],
            categories: Vec::new(),
            yanked: false,
            deprecated: false,
            users: 5.0,
            deps: Vec::new(),
            owners: vec![user(1)],
        }
    }
}

/// The dump, usage and candidates of a set of [`Spec`]s.
pub(super) struct World {
    pub(super) snapshot: DumpSnapshot,
    pub(super) usage: IdMap<Usage>,
    pub(super) candidates: Vec<crate::discover::Candidate>,
}

impl World {
    pub(super) fn new(specs: Vec<Spec>) -> Self {
        let mut world = Self {
            snapshot: DumpSnapshot {
                meta: DumpMeta {
                    timestamp: Timestamp::UNIX_EPOCH,
                    crates_io_commit: String::new(),
                    directory: String::new(),
                },
                crates: IdMap::default(),
                versions: IdMap::default(),
                default_version: IdMap::default(),
                edges: Vec::new(),
                default_requirements: IdMap::default(),
                owners: IdMap::default(),
                users: IdMap::default(),
                teams: IdMap::default(),
                categories: IdMap::default(),
                keywords: IdMap::default(),
                deleted: Vec::new(),
                downloads: IdMap::default(),
                warnings: Default::default(),
            },
            usage: IdMap::default(),
            candidates: Vec::new(),
        };
        for spec in specs {
            world.add(spec);
        }
        world
    }

    fn add(&mut self, spec: Spec) {
        let row = CrateRow {
            name: spec.name.to_owned(),
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: Timestamp::UNIX_EPOCH,
            description: spec.description,
            homepage: spec.homepage.map(str::to_owned),
            repository: spec.repository,
            deprecation: None,
            has_readme: true,
        };
        self.snapshot.crates.insert(spec.id, row);
        self.snapshot.versions.insert(
            spec.id,
            VersionRow {
                crate_id: spec.id,
                num: "1.0.0".to_owned(),
                created_at: Timestamp::UNIX_EPOCH,
                yanked: spec.yanked,
                rust_version: None,
                edition: None,
                published_by: None,
            },
        );
        self.snapshot.default_version.insert(spec.id, spec.id);
        self.snapshot.default_requirements.insert(
            spec.id,
            spec.deps
                .iter()
                .map(|(crate_id, kind)| Requirement {
                    req: "^1".to_owned(),
                    crate_id: *crate_id,
                    kind: *kind,
                    optional: false,
                })
                .collect(),
        );
        self.snapshot.owners.insert(spec.id, spec.owners);
        self.snapshot.keywords.insert(
            spec.id,
            spec.keywords.iter().map(|k| (*k).to_owned()).collect(),
        );
        self.snapshot.categories.insert(
            spec.id,
            spec.categories.iter().map(|c| (*c).to_owned()).collect(),
        );
        self.usage.insert(
            spec.id,
            Usage {
                current: spec.users,
                peak: spec.users,
                former_glory: 1.0,
                growth: 1.0,
                new_adopters: 0.0,
                removed: 0,
                expired: 0,
                history: Vec::new(),
            },
        );
        self.candidates.push(crate::discover::Candidate {
            crate_id: spec.id,
            name: spec.name.to_owned(),
            reasons: Vec::new(),
            deprecated: spec.deprecated,
        });
    }
}
