//! The repo link checks of `catalogue tiers`.

use std::collections::BTreeMap;
use std::path::Path;

use catalogue_core::{Catalogue, EntryKind, Slug};
use catalogue_signals::assemble::{CrateIndex, primary_release};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::github::{GITHUB_INTERVAL, GithubError, RepoRef};
use catalogue_signals::http::{IntervalPacer, Pacer, Transport};
use catalogue_signals::link::{CrateRelease, LinkCheck, LinkError, cached_link, check_link};

use super::collect::github_repo;
use crate::batch;

/// A library with a GitHub repo and the release to check.
type Target = (Slug, RepoRef, CrateRelease);

/// The repo link results, keyed by slug.
#[derive(Debug, Default)]
pub(super) struct Links {
    /// The result of each library that has one.
    pub(super) checks: BTreeMap<Slug, LinkCheck>,
    /// Why a library has no result, when its check fails on that library alone.
    pub(super) notes: BTreeMap<Slug, String>,
}

/// Checks the repo link of every library entry.
///
/// A library whose repo is not on GitHub gets [`LinkCheck::UnsupportedHost`]. A
/// library with a GitHub repo and a release in `snapshot` gets
/// [`check_link`] on its latest unyanked version, which reads and writes the
/// link cache under `cache_dir`. A library with no such release gets no result.
/// A library whose package is missing or unreadable gets a note and no result.
/// Token errors, rate-limit errors, transport errors and cache errors end the run.
pub(super) fn fetch_links(
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    transport: &dyn Transport,
    token: &str,
    cache_dir: &Path,
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
                links
                    .checks
                    .insert(slug.clone(), LinkCheck::UnsupportedHost);
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
        &targets, transport, token, cache_dir, &mut pacer, &mut links,
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
    pacer: &mut dyn Pacer,
    links: &mut Links,
) -> Result<(), LinkError> {
    let mut misses: Vec<&Target> = Vec::new();
    for target in targets {
        let (slug, repo, release) = target;
        match cached_link(cache_dir, repo, release) {
            Ok(Some(outcome)) => {
                links.checks.insert(slug.clone(), outcome.check);
            }
            Ok(None) => misses.push(target),
            Err(error) => {
                links.notes.insert(slug.clone(), entry_note(error)?);
            }
        }
    }
    let checked = batch::each(&misses, pacer, |target, pacer| {
        let (_, repo, release) = &**target;
        match check_link(transport, token, repo, release, cache_dir, pacer) {
            Ok(outcome) => Ok(Ok(outcome.check)),
            Err(error) => entry_note(error).map(Err),
        }
    })?;
    for (target, result) in misses.iter().zip(checked) {
        let (slug, ..) = target;
        match result {
            Ok(check) => {
                links.checks.insert(slug.clone(), check);
            }
            Err(note) => {
                links.notes.insert(slug.clone(), note);
            }
        }
    }
    Ok(())
}

/// The note for `error` when it concerns one library alone, so the other
/// libraries still run. Any other error returns as is and ends the run.
fn entry_note(error: LinkError) -> Result<String, LinkError> {
    if fails_one_entry(&error) {
        Ok(format!("repo link unchecked: {error}"))
    } else {
        Err(error)
    }
}

/// Whether `error` concerns one library alone, so the other libraries still run.
fn fails_one_entry(error: &LinkError) -> bool {
    match error {
        LinkError::Download { .. } | LinkError::Package { .. } | LinkError::UnsafeName { .. } => {
            true
        }
        LinkError::Github(GithubError::Status { status, .. }) => {
            !matches!(*status, 401 | 403 | 429)
        }
        LinkError::Github(GithubError::Body { .. } | GithubError::Json { .. }) => true,
        LinkError::Http(_) | LinkError::Github(_) | LinkError::Cache { .. } => false,
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
            )
            .expect("stores");
        }
        let transport = Fixed {
            status,
            urls: RefCell::new(Vec::new()),
        };
        let mut pacer = Counting::default();
        let mut links = Links::default();
        let result = check_targets(targets, &transport, "t", dir.path(), &mut pacer, &mut links);
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
    fn a_server_error_on_the_package_is_a_note_too() {
        let (links, _, _, result) = run(500, &[target("aa")], &[]);
        result.expect("a 500 on one package does not end the run");
        assert_eq!(links.notes.len(), 1);
    }

    #[test]
    fn errors_for_one_library_are_notes_and_token_errors_end_the_run() {
        let download = LinkError::Download {
            url: "u".to_owned(),
            status: 404,
        };
        assert!(entry_note(download).is_ok());
        let github = |status| {
            LinkError::Github(GithubError::Status {
                url: "u".to_owned(),
                status,
                action: "rerun later",
            })
        };
        for status in [401, 403, 429] {
            assert!(entry_note(github(status)).is_err(), "{status}");
        }
        assert!(entry_note(github(500)).is_ok());
        let transport = LinkError::Http(HttpError::Client {
            reason: "down".to_owned(),
        });
        assert!(entry_note(transport).is_err());
    }
}
