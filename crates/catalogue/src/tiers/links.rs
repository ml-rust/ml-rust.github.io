//! The repo link checks of `catalogue tiers`.

use std::collections::BTreeMap;
use std::path::Path;

use catalogue_core::{Catalogue, Date, EntryKind, Slug};
use catalogue_signals::assemble::{CrateIndex, primary_release};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::fallback::{Failed, attempt, link_failure};
use catalogue_signals::github::{GITHUB_INTERVAL, RepoRef};
use catalogue_signals::history::Dated;
use catalogue_signals::http::{IntervalPacer, Pacer, Transport};
use catalogue_signals::link::{
    CrateRelease, LinkCheck, LinkError, LinkOutcome, cached_link, check_link,
};

use super::collect::github_repo;
use crate::batch;

/// A library with a GitHub repo and the release to check.
type Target = (Slug, RepoRef, CrateRelease);

/// The repo link results, keyed by slug.
#[derive(Debug, Default)]
pub(super) struct Links {
    /// The result of each library that has one, with its check date.
    pub(super) checks: BTreeMap<Slug, Dated<LinkCheck>>,
    /// Why a library has no result, when the answer is final.
    pub(super) notes: BTreeMap<Slug, String>,
    /// The checks that failed and fall back to the previous snapshot.
    pub(super) failures: BTreeMap<Slug, LinkFailure>,
    /// The checks that sent a request. A cache hit is no fetch.
    pub(super) fetches: usize,
}

/// A link check that failed and falls back.
#[derive(Debug)]
pub(super) struct LinkFailure {
    /// What the check asked about.
    pub(super) of: String,
    /// The error text.
    pub(super) error: String,
}

/// What a link result describes: the crate release and the repo.
fn subject(repo: &RepoRef, release: &CrateRelease) -> String {
    let CrateRelease { name, version } = release;
    format!("{name} {version} in {repo}")
}

fn dated(of: String, outcome: LinkOutcome) -> Dated<LinkCheck> {
    Dated {
        date: outcome.checked,
        of,
        value: outcome.check,
    }
}

/// Checks the repo link of every library entry.
///
/// A library whose repo is not on GitHub gets [`LinkCheck::UnsupportedHost`]. A
/// library with a GitHub repo and a release in `snapshot` gets
/// [`check_link`] on its latest unyanked version, which reads and writes the
/// link cache under `cache_dir`. A new check is dated `today`, a cached one
/// keeps its stored date. A library with no such release gets no result.
/// [`link_failure`] sorts every error. A missing package gets a note and no
/// result. A failed fetch lands in [`Links::failures`]. A rejected token and a
/// cache error end the run.
pub(super) fn fetch_links(
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    transport: &dyn Transport,
    token: &str,
    cache_dir: &Path,
    today: Date,
) -> Result<Links, LinkError> {
    let index = CrateIndex::new(snapshot);
    let mut links = Links::default();
    let mut targets: Vec<Target> = Vec::new();
    for (slug, entry) in catalogue.entries() {
        if entry.kind != EntryKind::Library {
            continue;
        }
        match github_repo(entry) {
            Err(_) => {
                let check = Dated {
                    date: today,
                    of: entry.repo.to_string(),
                    value: LinkCheck::UnsupportedHost,
                };
                links.checks.insert(slug.clone(), check);
            }
            Ok(repo) => {
                if let Some(release) = primary_release(entry, snapshot, &index) {
                    targets.push((slug.clone(), repo, release));
                }
            }
        }
    }
    let mut pacer = IntervalPacer::new(GITHUB_INTERVAL);
    check_targets(
        &targets, transport, token, cache_dir, today, &mut pacer, &mut links,
    )?;
    Ok(links)
}

/// Checks `targets` into `links`.
///
/// The cache answers first, with no request and no wait. The `pacer` spaces
/// only the checks that miss the cache.
fn check_targets(
    targets: &[Target],
    transport: &dyn Transport,
    token: &str,
    cache_dir: &Path,
    today: Date,
    pacer: &mut dyn Pacer,
    links: &mut Links,
) -> Result<(), LinkError> {
    let mut misses: Vec<&Target> = Vec::new();
    for target in targets {
        let (slug, repo, release) = target;
        match attempt(cached_link(cache_dir, repo, release), link_failure)? {
            Ok(Some(outcome)) => {
                let check = dated(subject(repo, release), outcome);
                links.checks.insert(slug.clone(), check);
            }
            Ok(None) => misses.push(target),
            Err(failed) => settle_failure(target, failed, links),
        }
    }
    let checked = batch::each(&misses, pacer, |target, pacer| {
        let (_, repo, release) = &**target;
        attempt(
            check_link(transport, token, repo, release, cache_dir, today, pacer),
            link_failure,
        )
    })?;
    links.fetches += misses.len();
    for (target, result) in misses.iter().zip(checked) {
        let (slug, repo, release) = target;
        match result {
            Ok(outcome) => {
                let check = dated(subject(repo, release), outcome);
                links.checks.insert(slug.clone(), check);
            }
            Err(failed) => settle_failure(target, failed, links),
        }
    }
    Ok(())
}

/// Files the check of `target` that ended without a result.
fn settle_failure(target: &Target, failed: Failed, links: &mut Links) {
    let (slug, repo, release) = target;
    match failed {
        Failed::Definitive(error) => {
            let note = format!("repo link unchecked: {error}");
            links.notes.insert(slug.clone(), note);
        }
        Failed::Fallback(error) => {
            let of = subject(repo, release);
            links
                .failures
                .insert(slug.clone(), LinkFailure { of, error });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io::Cursor;

    use catalogue_signals::http::{Headers, HttpError, Response};
    use catalogue_signals::link::store_check;

    use super::*;

    /// Answers every request with `status` and records the URLs.
    struct Fixed {
        status: u16,
        urls: RefCell<Vec<String>>,
    }

    impl Transport for Fixed {
        fn get(&self, url: &str, _headers: &[(&str, String)]) -> Result<Response, HttpError> {
            self.urls.borrow_mut().push(url.to_owned());
            Ok(Response {
                status: self.status,
                headers: Headers::default(),
                body: Box::new(Cursor::new(Vec::new())),
            })
        }

        fn post(
            &self,
            url: &str,
            headers: &[(&str, String)],
            _body: &[u8],
        ) -> Result<Response, HttpError> {
            self.get(url, headers)
        }

        fn patch(
            &self,
            url: &str,
            headers: &[(&str, String)],
            _body: &[u8],
        ) -> Result<Response, HttpError> {
            self.get(url, headers)
        }

        fn delete(
            &self,
            url: &str,
            headers: &[(&str, String)],
            _body: &[u8],
        ) -> Result<Response, HttpError> {
            self.get(url, headers)
        }
    }

    /// Counts the waits.
    #[derive(Default)]
    struct Counting {
        waits: usize,
    }

    impl Pacer for Counting {
        fn wait(&mut self, _response_headers: &Headers) {
            self.waits += 1;
        }
    }

    fn today() -> Date {
        Date::new(2026, 11, 3).expect("date")
    }

    /// The check date of every cached result.
    fn stored() -> Date {
        Date::new(2026, 10, 1).expect("date")
    }

    fn target(name: &str) -> Target {
        (
            Slug::new(name).expect("slug"),
            RepoRef::parse(&format!("o/{name}")).expect("repo"),
            CrateRelease {
                name: name.to_owned(),
                version: "1.0.0".to_owned(),
            },
        )
    }

    fn run(
        status: u16,
        targets: &[Target],
        cached: &[&str],
    ) -> (Links, Vec<String>, usize, Result<(), LinkError>) {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        std::fs::create_dir_all(&base).expect("target dir");
        let dir = tempfile::tempdir_in(&base).expect("temp dir");
        for name in cached {
            let (_, repo, release) = target(name);
            store_check(
                dir.path(),
                &release,
                &repo.to_string(),
                &LinkCheck::NoVcsInfo,
                stored(),
            )
            .expect("stores");
        }
        let transport = Fixed {
            status,
            urls: RefCell::new(Vec::new()),
        };
        let mut pacer = Counting::default();
        let mut links = Links::default();
        let result = check_targets(
            targets,
            &transport,
            "t",
            dir.path(),
            today(),
            &mut pacer,
            &mut links,
        );
        let urls = transport.urls.borrow().clone();
        (links, urls, pacer.waits, result)
    }

    #[test]
    fn cache_hits_send_no_request_and_cause_no_wait() {
        let targets = [target("aa"), target("bb"), target("cc")];
        let (links, urls, waits, result) = run(500, &targets, &["aa", "bb", "cc"]);
        result.expect("hits");
        assert!(urls.is_empty());
        assert_eq!(waits, 0);
        assert_eq!(links.checks.len(), 3);
        assert!(links.checks.values().all(|check| check.date == stored()));
    }

    #[test]
    fn the_pacer_spaces_only_the_misses() {
        let targets = [target("aa"), target("bb"), target("cc"), target("dd")];
        let (links, urls, waits, result) = run(404, &targets, &["aa", "cc"]);
        result.expect("package errors are notes");
        assert_eq!(urls.len(), 2);
        assert_eq!(waits, 1);
        assert_eq!(links.checks.len(), 2);
        assert_eq!(links.notes.len(), 2);
    }

    #[test]
    fn a_missing_package_is_a_note_and_not_a_result() {
        let targets = [target("aa")];
        let (links, _, _, result) = run(404, &targets, &[]);
        result.expect("a 404 does not end the run");
        assert!(links.checks.is_empty());
        let note = links.notes.values().next().expect("note");
        assert!(note.starts_with("repo link unchecked: "), "{note}");
        assert!(note.contains("status 404"), "{note}");
    }

    #[test]
    fn a_server_error_on_the_package_falls_back() {
        let (links, urls, _, result) = run(500, &[target("aa")], &[]);
        result.expect("a 500 on one package does not end the run");
        assert_eq!(urls.len(), 1);
        assert!(links.notes.is_empty());
        assert_eq!(links.fetches, 1);
        let failure = links.failures.values().next().expect("failure");
        assert_eq!(failure.of, "aa 1.0.0 in o/aa");
        assert!(failure.error.contains("status 500"), "{}", failure.error);
    }

    #[test]
    fn a_cache_hit_is_no_fetch() {
        let (links, _, _, result) = run(404, &[target("aa"), target("bb")], &["aa"]);
        result.expect("runs");
        assert_eq!(links.fetches, 1);
        let check = links.checks.values().next().expect("hit");
        assert_eq!(check.of, "aa 1.0.0 in o/aa");
    }
}
