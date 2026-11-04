//! Hand-built dumps, entries and thresholds for the assembly tests.

use catalogue_core::{Date, Deprecated, Entry, Thresholds};
use jiff::Timestamp;
use jiff::civil;
use jiff::tz::TimeZone;

use super::assembled::Assembled;
use super::index::CrateIndex;
use super::library::library_signals;
use super::sources::LibrarySources;
use crate::dump::{
    CrateRow, DailyDownloads, DeletedCrate, DependencyKind, DumpMeta, DumpSnapshot, IdMap,
    Requirement, VersionRow,
};
use crate::github::{Facts, RepoFacts};
use crate::history::RepoValues;
use crate::link::LinkCheck;
use crate::rustsec::{Advisories, Advisory, AdvisoryKind};
use crate::sandbox::BuildChecks;
use crate::usage::Usage;

/// The published thresholds file.
const THRESHOLDS: &str = include_str!("../../../../data/thresholds.toml");

/// A civil date.
pub(crate) fn day(year: i16, month: i8, day: i8) -> civil::Date {
    civil::date(year, month, day)
}

/// The check date of every test.
pub(crate) fn today() -> Date {
    Date::from(day(2026, 11, 3))
}

/// The published thresholds.
pub(crate) fn thresholds() -> Thresholds {
    toml::from_str(THRESHOLDS).expect("thresholds parse")
}

/// A library entry that lists `crates`, primary first.
pub(crate) fn library(crates: &[&str]) -> Entry {
    let list: Vec<String> = crates.iter().map(|name| format!("\"{name}\"")).collect();
    entry(&format!(
        "kind = \"library\"\ncrates = [{}]\n{BODY}",
        list.join(", ")
    ))
}

const BODY: &str = r#"name = "Tool"
repo = "https://github.com/ml-rust/tool"
topics = ["ml"]
added = 2026-01-01

[description]
text = "A tool."
source = "human"
"#;

fn entry(text: &str) -> Entry {
    toml::from_str(text).expect("entry parses")
}

/// The facts of a live repo with one unanswered issue.
pub(crate) fn found_facts() -> Facts {
    Facts {
        name_with_owner: "ml-rust/tool".to_owned(),
        moved: false,
        archived: false,
        fork: false,
        pushed_at: None,
        stars: 0,
        ci: None,
        last_human_commit: Some(day(2026, 10, 1)),
        last_human_commit_beyond_sample: false,
        human_commits_365: 4,
        human_commits_capped: false,
        oldest_unanswered_days: Some(12),
        maintainer_replies_365: 7,
        median_first_response_days: None,
        items_sampled: 9,
    }
}

/// A dump under construction.
pub(crate) struct Dump {
    pub(crate) snapshot: DumpSnapshot,
}

impl Dump {
    /// An empty dump taken on [`today`].
    pub(crate) fn new() -> Self {
        let timestamp = day(2026, 11, 3)
            .to_zoned(TimeZone::UTC)
            .expect("zoned")
            .timestamp();
        Self {
            snapshot: DumpSnapshot {
                meta: DumpMeta {
                    timestamp,
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
            },
        }
    }

    /// Adds crate `id` named `name` with a README and no notice.
    pub(crate) fn add_crate(&mut self, id: u32, name: &str) -> &mut CrateRow {
        self.snapshot.crates.entry(id).or_insert(CrateRow {
            name: name.to_owned(),
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: Timestamp::UNIX_EPOCH,
            description: None,
            homepage: None,
            repository: None,
            deprecation: None,
            has_readme: true,
        })
    }

    /// Adds version `id` of crate `crate_id`, published at midday UTC on `date`.
    pub(crate) fn add_version(
        &mut self,
        id: u32,
        crate_id: u32,
        num: &str,
        date: civil::Date,
        yanked: bool,
    ) {
        let created_at = date
            .at(12, 0, 0, 0)
            .to_zoned(TimeZone::UTC)
            .expect("zoned")
            .timestamp();
        let row = VersionRow {
            crate_id,
            num: num.to_owned(),
            created_at,
            yanked,
            rust_version: None,
            edition: None,
            published_by: None,
            has_lib: Some(true),
            has_bin: false,
        };
        self.snapshot.versions.insert(id, row);
    }

    /// Makes version `version_id` the default version of crate `crate_id`.
    pub(crate) fn set_default(&mut self, crate_id: u32, version_id: u32) {
        self.snapshot.default_version.insert(crate_id, version_id);
    }

    /// Adds a requirement `req` of kind `kind` on crate `on` to crate `crate_id`'s default version.
    pub(crate) fn add_requirement(
        &mut self,
        crate_id: u32,
        on: u32,
        req: &str,
        kind: DependencyKind,
    ) {
        let requirement = Requirement {
            req: req.to_owned(),
            crate_id: on,
            kind,
            optional: false,
        };
        self.snapshot
            .default_requirements
            .entry(crate_id)
            .or_default()
            .push(requirement);
    }

    /// Adds a `deleted_crates` row for `name`.
    pub(crate) fn add_deleted(&mut self, name: &str) {
        self.snapshot.deleted.push(DeletedCrate {
            name: name.to_owned(),
            deleted_at: Timestamp::UNIX_EPOCH,
        });
    }

    /// Sets the daily downloads of crate `crate_id`, window starting on `start`.
    pub(crate) fn set_downloads(&mut self, crate_id: u32, start: civil::Date, days: Vec<u32>) {
        self.snapshot
            .downloads
            .insert(crate_id, DailyDownloads { start, days });
    }
}

/// A dump, usage, advisories and deprecated list under construction.
pub(crate) struct World {
    pub(crate) dump: Dump,
    usage: IdMap<Usage>,
    advisories: Advisories,
    deprecated: Deprecated,
    pub(crate) build_checks: BuildChecks,
}

impl World {
    pub(crate) fn new() -> Self {
        let deprecated = toml::from_str(
            "[[requirement]]\ncrate = \"tokio\"\nbelow = \"1.0.0\"\nreason = \"r\"\nsource = \"https://example.com/r\"\n",
        )
        .expect("list parses");
        Self {
            dump: Dump::new(),
            usage: IdMap::default(),
            advisories: Advisories::default(),
            deprecated,
            build_checks: BuildChecks::default(),
        }
    }

    pub(crate) fn run(&self, crates: &[&str], facts: Option<&RepoFacts>) -> Assembled {
        self.assemble(crates, facts, None)
    }

    pub(crate) fn run_linked(&self, crates: &[&str], link: Option<&LinkCheck>) -> Assembled {
        self.assemble(crates, None, link)
    }

    fn assemble(
        &self,
        crates: &[&str],
        facts: Option<&RepoFacts>,
        link: Option<&LinkCheck>,
    ) -> Assembled {
        let index = CrateIndex::new(&self.dump.snapshot);
        let thresholds = thresholds();
        let sources = LibrarySources {
            dump: &self.dump.snapshot,
            index: &index,
            usage: &self.usage,
            advisories: &self.advisories,
            deprecated: &self.deprecated,
            thresholds: &thresholds,
            build_checks: &self.build_checks,
        };
        let values = facts.map(RepoValues::from);
        library_signals(&library(crates), &sources, values.as_ref(), link, today())
    }

    pub(crate) fn users(&mut self, id: u32, current: f64) {
        let usage = Usage {
            current,
            peak: current * 2.0,
            former_glory: 0.5,
            growth: 1.0,
            new_adopters: current / 2.0,
            removed: 0,
            expired: 0,
            history: Vec::new(),
        };
        self.usage.insert(id, usage);
    }

    pub(crate) fn advisory(&mut self, id: &str, kind: AdvisoryKind, date: civil::Date) {
        let advisory = Advisory {
            id: id.to_owned(),
            package: "main".to_owned(),
            date,
            kind,
            withdrawn: None,
            patched: Vec::new(),
            unaffected: Vec::new(),
            url: None,
        };
        let by_crate = &mut self.advisories.by_crate;
        by_crate
            .entry("main".to_owned())
            .or_default()
            .push(advisory);
    }
}
