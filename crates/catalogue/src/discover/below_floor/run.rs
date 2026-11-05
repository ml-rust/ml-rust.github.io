//! The run of `catalogue discover below-floor`: the inputs, the fetch and the lead files.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_core::normalize_repo_url;
use catalogue_signals::discover;
use catalogue_signals::dump::{IdSet, utc_date};
use catalogue_signals::fetch;
use catalogue_signals::github::{GITHUB_INTERVAL, GithubError, RepoRef, fetch_repo_stats};
use catalogue_signals::http::{IntervalPacer, ReqwestTransport};
use catalogue_signals::seed::ExistingIndex;
use catalogue_signals::usage;

use super::render::{BelowFloorReport, Counts, build_below_floor_report};
use crate::cache_dir::resolve_cache_dir;
use crate::clock::today_utc;
use crate::discover::command::{crate_rules, load_catalogue, report_error};
use crate::discover::error::DiscoverError;
use crate::dump::{DumpCommandError, read_cached};
use crate::exit::IO;
use crate::seed::{LEAD_FILE, SeedError, parse_leads};
use crate::token::{NoToken, token_from_env};

/// The file of repos reviewed and left out, so the report does not repeat them.
const REVIEWED_FILE: &str = "leads-reviewed.txt";

/// Runs `discover below-floor` and returns the process exit code.
pub(in crate::discover) fn run_below_floor(
    data: &Path,
    cache_dir: Option<PathBuf>,
    window_days: u16,
    min_users: f64,
) -> ExitCode {
    match discover_below_floor(data, cache_dir, window_days, min_users, token_from_env()) {
        Ok(report) => {
            for warning in &report.warnings {
                eprintln!("warning: {warning}");
            }
            println!("{}", report.text);
            if report.unfetched == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(IO)
            }
        }
        Err(error) => report_error(error),
    }
}

/// Checks the data, reads the cached dump, fetches the repos and reports the leads.
fn discover_below_floor(
    data: &Path,
    cache_dir: Option<PathBuf>,
    window_days: u16,
    min_users: f64,
    token: Result<String, NoToken>,
) -> Result<BelowFloorReport, DiscoverError> {
    let token = token?;
    let catalogue = load_catalogue(data)?;
    let rules = crate_rules(&catalogue, data)?;
    let limits = catalogue
        .discovery()
        .and_then(|discovery| discovery.below_floor)
        .ok_or_else(|| DiscoverError::NoBelowFloor {
            path: data.join("discovery.toml"),
        })?;
    let mut listed = read_leads(&data.join(LEAD_FILE))?;
    listed.extend(read_leads(&data.join(REVIEWED_FILE))?);
    let known = known_repos(ExistingIndex::from_catalogue(&catalogue), &listed);
    let cache_dir = resolve_cache_dir(cache_dir).map_err(DumpCommandError::from)?;
    let snapshot = read_cached(&fetch::dump_path(&cache_dir), window_days)?;
    let found = discover::candidates(&snapshot, &rules);
    let targets: IdSet = found.candidates.iter().map(|c| c.crate_id).collect();
    let signals = usage::compute(&snapshot, &targets, utc_date(snapshot.meta.timestamp));
    let grouped = discover::group_repos(
        &found.candidates,
        |id| {
            snapshot
                .crates
                .get(&id)
                .and_then(|row| row.repository.as_deref())
        },
        |id| signals.get(&id).map_or(0.0, |signal| signal.current),
        min_users,
    );
    let repos = grouped.groups.len();
    let groups = discover::drop_known(grouped.groups, &known);
    let transport = ReqwestTransport::new().map_err(GithubError::from)?;
    let asked: Vec<RepoRef> = groups.iter().map(|group| group.repo.clone()).collect();
    let fetched = fetch_repo_stats(
        &transport,
        &token,
        &asked,
        &mut IntervalPacer::new(GITHUB_INTERVAL),
    );
    let counts = Counts {
        crates: grouped.crates,
        repos,
        known: repos - groups.len(),
    };
    Ok(build_below_floor_report(
        counts,
        groups,
        fetched,
        &limits,
        today_utc(),
        &known,
    ))
}

/// The index plus the repos of `leads`, so a lead counts as known.
fn known_repos(index: ExistingIndex, leads: &[RepoRef]) -> ExistingIndex {
    leads.iter().fold(index, |index, lead| {
        match normalize_repo_url(&format!("github.com/{lead}")) {
            Some(key) => index.with_repo(&key, LEAD_FILE),
            None => index,
        }
    })
}

/// Reads a file of `owner/repo` lines. A missing file or a file with no repo gives none.
fn read_leads(path: &Path) -> Result<Vec<RepoRef>, DiscoverError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|source| DiscoverError::ReadLeads {
        path: path.to_path_buf(),
        source,
    })?;
    match parse_leads(path, &text) {
        Ok(leads) => Ok(leads),
        Err(SeedError::NoLeads { .. }) => Ok(Vec::new()),
        Err(error) => Err(DiscoverError::Leads(Box::new(error))),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};
    use crate::discover::DiscoverCommand;

    fn repo(name: &str) -> RepoRef {
        RepoRef::parse(name).expect("repo")
    }

    #[test]
    fn leads_count_as_known_in_any_case() {
        let known = known_repos(ExistingIndex::default(), &[repo("Owner/Repo")]);
        assert!(known.repo_slug("github.com/owner/repo").is_some());
        assert!(known.repo_slug("github.com/other/repo").is_none());
    }

    #[test]
    fn the_lead_file_is_optional_and_a_comment_only_file_holds_no_lead() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join(LEAD_FILE);
        assert!(read_leads(&path).expect("none").is_empty());
        fs::write(&path, "# only a comment\n").expect("write");
        assert!(read_leads(&path).expect("none").is_empty());
        fs::write(&path, "# c\nA/b\nc/d\tignored\n").expect("write");
        let leads = read_leads(&path).expect("leads");
        let names: Vec<String> = leads.iter().map(ToString::to_string).collect();
        assert_eq!(names, ["A/b", "c/d"]);
    }

    #[test]
    fn a_bad_lead_line_names_the_line() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join(LEAD_FILE);
        fs::write(&path, "a/b\nnope\n").expect("write");
        let error = read_leads(&path).expect_err("bad lead");
        let text = error.to_string();
        assert!(text.contains("`nope`") && text.contains(":2:"), "{text}");
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn the_committed_data_holds_the_below_floor_limits() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let catalogue = load_catalogue(&data).expect("valid");
        let limits = catalogue
            .discovery()
            .and_then(|discovery| discovery.below_floor)
            .expect("the [below_floor] table");
        assert_eq!((limits.min_stars, limits.active_days), (100, 730));
    }

    #[test]
    fn a_missing_table_names_the_file_and_the_next_action() {
        let error = DiscoverError::NoBelowFloor {
            path: PathBuf::from("data/discovery.toml"),
        };
        let text = error.to_string();
        assert!(
            text.contains("data/discovery.toml") && text.contains("then rerun"),
            "{text}"
        );
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn discover_below_floor_has_defaults_and_takes_options() {
        let cli = Cli::try_parse_from(["catalogue", "discover", "below-floor"]).expect("parses");
        let Command::Discover {
            command:
                DiscoverCommand::BelowFloor {
                    data,
                    cache_dir,
                    window_days,
                    min_users,
                },
        } = cli.command
        else {
            panic!("expected discover below-floor");
        };
        assert_eq!(
            (data, cache_dir, window_days, min_users),
            (PathBuf::from("data"), None, 90, 2.0)
        );
        let cli =
            Cli::try_parse_from(["catalogue", "discover", "below-floor", "--min-users", "1.5"])
                .expect("parses");
        let Command::Discover {
            command: DiscoverCommand::BelowFloor { min_users, .. },
        } = cli.command
        else {
            panic!("expected discover below-floor");
        };
        assert_eq!(min_users, 1.5);
        assert!(
            Cli::try_parse_from(["catalogue", "discover", "below-floor", "--min-users", "-1"])
                .is_err()
        );
    }
}
