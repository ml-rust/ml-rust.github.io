//! One `catalogue tiers` run over loaded inputs: fetch, check the failed share, assess, write.

use std::path::Path;

use catalogue_core::{Catalogue, Date};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::github::GITHUB_INTERVAL;
use catalogue_signals::history::{History, ReadmeStore, Snapshot, read_all, write};
use catalogue_signals::http::{IntervalPacer, Pacer};
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::BuildChecks;

use super::assess::assess_entries;
use super::cadence::FetchMode;
use super::collect::{Network, fetch_all};
use super::command::required;
use super::error::TiersError;
use super::inputs::{Inputs, Outputs, TiersRun};
use super::render::render_tiers;
use super::report::{tier_report, write_report};
use super::stored::{Memory, readme_store};

/// The `catalogue tiers` report for a checked catalogue on `today`.
///
/// Fetches the network signals through `network`, assembles each entry's
/// signals, applies the tier rules and renders one block per entry, in slug order.
/// The same pass builds the [`TierReport`](catalogue_core::TierReport), with the source and data
/// date of each signal.
/// `build_checks` holds the sandbox results that rot signal R6 reads.
///
/// With `history_dir`, the run reads the snapshots in that directory before it
/// applies the rules, then writes the snapshot of `today` and the README store to it. Without it,
/// the run reads and writes no history.
///
/// A network fetch that fails falls back to the previous snapshot, and an online
/// run ends its text with the `fallbacks:` block. Every value keeps the date of its data.
///
/// # Errors
///
/// Returns [`TiersError::MissingFile`] when the catalogue lacks
/// `thresholds.toml` or `deprecated.toml`, the fatal request error that ends an
/// online run, such as a rejected token, [`TiersError::History`] when the
/// history directory cannot be read or written, and [`TiersError::TooManyFailed`]
/// when the share of fetches with no usable earlier value exceeds
/// `fallback.max_failed_share`, in a run of at least
/// [`MIN_FETCHES_FOR_FAILED_SHARE`](catalogue_signals::fallback::MIN_FETCHES_FOR_FAILED_SHARE)
/// fetches.
pub fn tiers_report(
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    advisories: &Advisories,
    build_checks: &BuildChecks,
    network: Network<'_>,
    history_dir: Option<&Path>,
    today: Date,
) -> Result<TiersRun, TiersError> {
    let inputs = Inputs {
        catalogue,
        snapshot,
        advisories,
        build_checks,
        fetch: FetchMode::All,
        today,
    };
    tiers_report_with(&inputs, network, history_dir)
}

/// The same report as [`tiers_report`], with the fetch mode of `inputs`.
///
/// # Errors
///
/// The errors of [`tiers_report`], and [`TiersError::NoStoredValues`] when `FetchMode::None` finds
/// no snapshot.
pub fn tiers_report_with(
    inputs: &Inputs<'_>,
    network: Network<'_>,
    history_dir: Option<&Path>,
) -> Result<TiersRun, TiersError> {
    let outputs = Outputs {
        history_dir,
        json: None,
    };
    run_with(
        inputs,
        network,
        &outputs,
        &mut IntervalPacer::new(GITHUB_INTERVAL),
    )
}

/// Runs [`tiers_report`] with the `github` pacer, then writes the JSON report to `outputs.json`.
///
/// A run over the failed share writes neither the snapshot nor the report. The report follows the
/// history write.
pub(super) fn run_with(
    inputs: &Inputs<'_>,
    network: Network<'_>,
    outputs: &Outputs<'_>,
    github: &mut dyn Pacer,
) -> Result<TiersRun, TiersError> {
    let Inputs {
        catalogue,
        snapshot,
        advisories,
        build_checks,
        fetch,
        today,
    } = *inputs;
    let (thresholds, _) = required(catalogue)?;
    let (history, readmes) = match outputs.history_dir {
        Some(dir) => (read_all(dir)?, ReadmeStore::read(dir)?),
        None => (History::default(), ReadmeStore::default()),
    };
    if let (FetchMode::None, true, Some(dir)) = (fetch, history.is_empty(), outputs.history_dir) {
        return Err(TiersError::NoStoredValues {
            path: dir.to_path_buf(),
        });
    }
    let online = matches!(network, Network::Online { .. });
    let memory = Memory {
        history: &history,
        readmes: &readmes,
        max_age_days: thresholds.fallback.max_age_days,
        fetch,
        today,
    };
    let fetched = fetch_all(catalogue, snapshot, network, memory, github)?;
    let limit = thresholds.fallback.max_failed_share;
    if fetched.summary.exceeds_lost_limit(limit) {
        return Err(TiersError::TooManyFailed {
            limit,
            summary: fetched.summary,
        });
    }
    let assessments = assess_entries(
        catalogue,
        snapshot,
        advisories,
        build_checks,
        &fetched.entries,
        &history,
        today,
    )?;
    if let Some(dir) = outputs.history_dir {
        let rows = assessments
            .iter()
            .map(|assessment| (assessment.slug.to_string(), assessment.row.clone()))
            .collect();
        readme_store(&fetched.entries).write(dir)?;
        write(dir, &Snapshot { date: today, rows })?;
    }
    let mut text = render_tiers(&assessments);
    if online {
        text.push_str("\n\n");
        text.push_str(&fetched.summary.text());
    }
    if fetch != FetchMode::All {
        let asked = fetched.entries.values().filter(|f| f.asked).count();
        text.push_str(&format!(
            "\n\nfetch mode {fetch}: asked about {asked} of {} entries",
            fetched.entries.len()
        ));
    }
    let run = TiersRun {
        text,
        report: tier_report(&assessments, thresholds, today),
        summary: fetched.summary,
    };
    if let Some(path) = outputs.json {
        write_report(path, &run.report)?;
    }
    Ok(run)
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::time::Duration;

    use catalogue_signals::dump::{DumpMeta, IdMap};
    use catalogue_signals::http::{Clock, Headers, HttpError, Response, Transport};
    use tempfile::TempDir;

    use super::*;
    use crate::data_dir::read_data_dir;
    use crate::exit::IO;

    /// A clock whose sleeps move its time and take no real time. Clones share the time.
    #[derive(Clone)]
    struct Fake(Rc<Cell<u64>>);

    impl Clock for Fake {
        fn now(&self) -> u64 {
            self.0.get()
        }

        fn sleep(&self, wait: Duration) {
            self.0.set(self.0.get() + wait.as_secs());
        }
    }

    /// Records each request with the time it left. With `fail` every request gets status 500.
    /// Otherwise each repo is found, every GET gets status 404 and the facts of `gamma` report a
    /// spent budget.
    struct Script {
        fail: bool,
        clock: Fake,
        seen: RefCell<Vec<(String, u64)>>,
    }

    impl Script {
        fn new(fail: bool, clock: Fake) -> Self {
            Self {
                fail,
                clock,
                seen: RefCell::new(Vec::new()),
            }
        }

        fn record(&self, url: &str) {
            let now = self.clock.now();
            self.seen.borrow_mut().push((url.to_owned(), now));
        }
    }

    fn reply(status: u16, headers: Headers, body: &str) -> Result<Response, HttpError> {
        Ok(Response {
            status,
            headers,
            body: Box::new(Cursor::new(body.as_bytes().to_vec())),
        })
    }

    impl Transport for Script {
        fn get(&self, url: &str, _: &[(&str, String)]) -> Result<Response, HttpError> {
            self.record(url);
            let status = if self.fail { 500 } else { 404 };
            reply(status, Headers::default(), "")
        }

        fn post(
            &self,
            url: &str,
            _: &[(&str, String)],
            body: &[u8],
        ) -> Result<Response, HttpError> {
            self.record(url);
            if self.fail {
                return reply(500, Headers::default(), "");
            }
            let found = r#"{"data":{"repository":{"nameWithOwner":"ml-rust/x"}}}"#;
            let headers = if String::from_utf8_lossy(body).contains("\"gamma\"") {
                Headers::new([
                    ("x-ratelimit-remaining", "0"),
                    ("x-ratelimit-reset", "3000"),
                ])
            } else {
                Headers::default()
            };
            reply(200, headers, found)
        }

        fn patch(&self, url: &str, _: &[(&str, String)], _: &[u8]) -> Result<Response, HttpError> {
            self.get(url, &[])
        }

        fn delete(&self, url: &str, _: &[(&str, String)], _: &[u8]) -> Result<Response, HttpError> {
            self.get(url, &[])
        }
    }

    fn fake_clock() -> Fake {
        Fake(Rc::new(Cell::new(1000)))
    }

    /// The fixture catalogue with `limit` as its failed share.
    fn catalogue(limit: &str) -> Catalogue {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid");
        let mut sources = read_data_dir(&dir).expect("fixture reads").sources;
        let thresholds = sources.thresholds.as_mut().expect("thresholds");
        let line = "max_failed_share = 1.0";
        assert!(thresholds.contents.contains(line), "fixture lacks {line}");
        thresholds.contents = thresholds
            .contents
            .replace(line, &format!("max_failed_share = {limit}"));
        Catalogue::load(sources).expect("fixture is valid")
    }

    /// The fixture catalogue with three more libraries, so a run reaches the failed share minimum.
    fn large_catalogue(limit: &str) -> Catalogue {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid");
        let mut sources = read_data_dir(&dir).expect("fixture reads").sources;
        let thresholds = sources.thresholds.as_mut().expect("thresholds");
        thresholds.contents = thresholds.contents.replace(
            "max_failed_share = 1.0",
            &format!("max_failed_share = {limit}"),
        );
        for slug in ["delta", "epsilon", "zeta"] {
            sources.entries.push(catalogue_core::SourceFile {
                path: format!("entries/{slug}.toml"),
                contents: format!(
                    "name = \"{slug}\"\nkind = \"library\"\nrepo = \"https://github.com/ml-rust/{slug}\"\n\
                     crates = [\"{slug}-crate\"]\ntopics = [\"ml\"]\nadded = 2026-11-03\n\n\
                     [description]\ntext = \"The {slug} project.\"\nsource = \"human\"\n"
                ),
            });
        }
        Catalogue::load(sources).expect("the catalogue is valid")
    }

    /// A dump that holds no crate, so no entry asks for a link check.
    fn empty_dump() -> DumpSnapshot {
        DumpSnapshot {
            meta: DumpMeta {
                timestamp: "2026-11-02T02:00:00Z".parse().expect("timestamp"),
                crates_io_commit: String::new(),
                directory: String::new(),
            },
            crates: IdMap::default(),
            versions: IdMap::default(),
            default_version: IdMap::default(),
            edges: Vec::new(),
            default_requirements: IdMap::default(),
            details: IdMap::default(),
            owners: IdMap::default(),
            users: IdMap::default(),
            teams: IdMap::default(),
            categories: IdMap::default(),
            keywords: IdMap::default(),
            deleted: Vec::new(),
            downloads: IdMap::default(),
            warnings: Default::default(),
        }
    }

    /// Paths under a temporary directory, and one run that writes to them.
    struct Run {
        dir: TempDir,
    }

    impl Run {
        fn new() -> Self {
            let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
            std::fs::create_dir_all(&base).expect("target dir");
            Self {
                dir: tempfile::tempdir_in(&base).expect("temp dir"),
            }
        }

        fn snapshot_file(&self) -> PathBuf {
            self.dir.path().join("history/2026-11-03.jsonl.gz")
        }

        fn report_file(&self) -> PathBuf {
            self.dir.path().join("tiers.json")
        }

        fn run(
            &self,
            catalogue: &Catalogue,
            transport: &dyn Transport,
            clock: Fake,
        ) -> Result<TiersRun, TiersError> {
            let snapshot = empty_dump();
            let advisories = Advisories {
                archive_date: Some(Date::new(2026, 11, 2).expect("date")),
                ..Advisories::default()
            };
            let build_checks = BuildChecks::default();
            let inputs = Inputs {
                catalogue,
                snapshot: &snapshot,
                advisories: &advisories,
                build_checks: &build_checks,
                fetch: FetchMode::All,
                today: Date::new(2026, 11, 3).expect("date"),
            };
            let history = self.dir.path().join("history");
            let report = self.report_file();
            let outputs = Outputs {
                history_dir: Some(&history),
                json: Some(&report),
            };
            let cache_dir = self.dir.path().join("cache");
            let network = Network::Online {
                transport,
                token: "sekret-xyz",
                cache_dir: &cache_dir,
            };
            let mut github = IntervalPacer::with_clock(GITHUB_INTERVAL, clock);
            run_with(&inputs, network, &outputs, &mut github)
        }
    }

    #[test]
    fn over_the_limit_the_run_fails_and_writes_nothing() {
        let run = Run::new();
        let clock = fake_clock();
        let script = Script::new(true, clock.clone());
        let error = run
            .run(&large_catalogue("0.5"), &script, clock)
            .expect_err("over the limit");
        let TiersError::TooManyFailed { limit, summary } = &error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(*limit, 0.5);
        assert!(summary.fetches > 0);
        assert_eq!(summary.failures.len(), summary.fetches);
        assert!(summary.lost_count() > 0);
        let text = error.to_string();
        let count = format!(
            "{} of {} network fetches",
            summary.lost_count(),
            summary.fetches
        );
        assert!(text.contains(&count), "{text}");
        assert!(text.contains("limit of 0.5"), "{text}");
        assert!(text.contains("gamma: "), "{text}");
        assert!(!text.contains("sekret-xyz"), "{text}");
        assert_eq!(error.exit_code(), IO);
        assert!(!run.snapshot_file().exists());
        assert!(!run.report_file().exists());
    }

    #[test]
    fn a_run_under_the_minimum_passes_and_leaves_lost_values_unknown() {
        let run = Run::new();
        let clock = fake_clock();
        let script = Script::new(true, clock.clone());
        let found = run
            .run(&catalogue("0.05"), &script, clock)
            .expect("too few fetches for the share to apply");
        assert!(found.summary.fetches < 20, "{}", found.summary.fetches);
        assert_eq!(found.summary.failures.len(), found.summary.fetches);
        assert!(run.snapshot_file().is_file());
        assert!(run.report_file().is_file());
    }

    #[test]
    fn exactly_at_the_limit_the_run_passes_and_writes_both_files() {
        let run = Run::new();
        let clock = fake_clock();
        let script = Script::new(true, clock.clone());
        let found = run
            .run(&catalogue("1.0"), &script, clock)
            .expect("every fetch failed, and the limit is every fetch");
        assert_eq!(found.summary.failures.len(), found.summary.fetches);
        assert!(run.snapshot_file().is_file());
        assert!(run.report_file().is_file());
    }

    #[test]
    fn a_spent_budget_holds_the_first_request_of_the_next_github_phase() {
        let run = Run::new();
        let clock = fake_clock();
        let script = Script::new(false, clock.clone());
        run.run(&catalogue("1.0"), &script, clock).expect("runs");
        let seen = script.seen.borrow();
        let graphql: Vec<u64> = seen
            .iter()
            .filter(|(url, _)| url.ends_with("/graphql"))
            .map(|(_, at)| *at)
            .collect();
        assert_eq!(graphql.len(), 3, "one facts request per entry");
        assert!(graphql.iter().all(|at| *at < 3000), "{graphql:?}");
        let later: Vec<&(String, u64)> = seen
            .iter()
            .filter(|(url, _)| url.starts_with("https://api.github.com/repos/"))
            .collect();
        assert!(!later.is_empty());
        assert!(later.iter().all(|(_, at)| *at > 3000), "{later:?}");
    }
}
