//! The report of `catalogue discover github`: repos with no crates.io crate.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_core::normalize_repo_url;
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::fetch;
use catalogue_signals::github::{
    FoundRepo, GITHUB_INTERVAL, GithubError, GithubFound, GithubRules, Repo, search_repos,
};
use catalogue_signals::http::{IntervalPacer, ReqwestTransport};

use super::command::{load_catalogue, report_error};
use super::error::DiscoverError;
use crate::cache_dir::resolve_cache_dir;
use crate::dump::{DumpCommandError, read_cached};
use crate::token::{NoToken, token_from_env};

/// Days of download history read from the dump. The link check needs none.
const LINK_WINDOW_DAYS: u16 = 1;

/// The result of `discover github`.
#[derive(Debug, PartialEq, Eq)]
pub struct GithubReport {
    /// The header and one line per lead.
    pub text: String,
    /// Searches that lost results.
    pub warnings: Vec<String>,
}

/// Runs `discover github` and returns the process exit code.
pub(super) fn run_github(data: &Path, cache_dir: Option<PathBuf>) -> ExitCode {
    match discover_github(data, cache_dir, token_from_env()) {
        Ok(report) => {
            for warning in &report.warnings {
                eprintln!("warning: {warning}");
            }
            println!("{}", report.text);
            ExitCode::SUCCESS
        }
        Err(error) => report_error(error),
    }
}

/// What one GitHub search found, with the links that filter and label it.
pub(crate) struct Searched {
    /// The repos the search returned.
    pub(crate) found: GithubFound,
    /// The normalised repository links of every crate in the dump.
    pub(crate) linked: HashSet<String>,
    /// Entry slugs by normalised repository link.
    pub(crate) listed: HashMap<String, String>,
}

/// Checks the data, reads the cached dump and searches GitHub.
pub(crate) fn search(
    data: &Path,
    cache_dir: Option<PathBuf>,
    token: &str,
) -> Result<Searched, DiscoverError> {
    let catalogue = load_catalogue(data)?;
    let path = data.join("discovery.toml");
    let discovery = catalogue
        .discovery()
        .ok_or_else(|| DiscoverError::NoRules { path: path.clone() })?;
    if discovery.github_topics.is_empty() {
        return Err(DiscoverError::NoTopics { path });
    }
    let rules = GithubRules {
        topics: discovery.github_topics.clone(),
        min_stars: discovery.github_min_stars,
    };
    let listed: HashMap<String, String> = catalogue
        .entries()
        .iter()
        .filter_map(|(slug, entry)| {
            normalize_repo_url(entry.repo.as_str()).map(|key| (key, slug.as_str().to_owned()))
        })
        .collect();
    let cache_dir = resolve_cache_dir(cache_dir).map_err(DumpCommandError::from)?;
    let snapshot = read_cached(&fetch::dump_path(&cache_dir), LINK_WINDOW_DAYS)?;
    let linked = linked_repos(&snapshot);
    let transport = ReqwestTransport::new().map_err(GithubError::from)?;
    let found = search_repos(
        &transport,
        token,
        &rules,
        &mut IntervalPacer::new(GITHUB_INTERVAL),
    )?;
    Ok(Searched {
        found,
        linked,
        listed,
    })
}

/// Checks the data, searches GitHub and reports the repos no crate links to.
fn discover_github(
    data: &Path,
    cache_dir: Option<PathBuf>,
    token: Result<String, NoToken>,
) -> Result<GithubReport, DiscoverError> {
    let token = token?;
    let searched = search(data, cache_dir, &token)?;
    Ok(build_github_report(
        &searched.found,
        &searched.linked,
        &searched.listed,
    ))
}

/// The normalised repository links of every crate in the dump.
fn linked_repos(snapshot: &DumpSnapshot) -> HashSet<String> {
    snapshot
        .crates
        .values()
        .filter_map(|row| row.repository.as_deref())
        .filter_map(normalize_repo_url)
        .collect()
}

/// The repos in `found` that no crate in `linked` points to, with their
/// normalised links, by stars descending, then name.
pub(crate) fn leads<'a>(
    found: &'a GithubFound,
    linked: &HashSet<String>,
) -> Vec<(&'a FoundRepo, String)> {
    let mut leads: Vec<(&FoundRepo, String)> = found
        .repos
        .iter()
        .filter_map(|item| {
            let key = normalize_repo_url(&item.repo.html_url)?;
            (!linked.contains(&key)).then_some((item, key))
        })
        .collect();
    leads.sort_by(|a, b| {
        b.0.repo
            .stargazers_count
            .cmp(&a.0.repo.stargazers_count)
            .then_with(|| a.0.repo.full_name.cmp(&b.0.repo.full_name))
    });
    leads
}

/// Renders the repos in `found` that no crate in `linked` points to.
///
/// `linked` and the keys of `listed` hold normalised repository links. `listed`
/// maps a link to the slug of the entry that holds it. Lines sort by stars
/// descending, then name. A line holds `full_name`, the stars, the date of the
/// last push, `topics:<a,b>` with the rule topics that found the repo, and
/// `listed:<slug>` for a repo an entry holds. The description is untrusted
/// text and stays out.
pub fn build_github_report(
    found: &GithubFound,
    linked: &HashSet<String>,
    listed: &HashMap<String, String>,
) -> GithubReport {
    let leads: Vec<(&Repo, Option<&String>, String)> = leads(found, linked)
        .into_iter()
        .map(|(item, key)| (&item.repo, listed.get(&key), item.matched_topics.join(",")))
        .collect();
    let mut lines = vec![format!("github leads: {}", leads.len())];
    for (repo, slug, topics) in leads {
        let pushed = repo.pushed_at.split('T').next().unwrap_or_default();
        let mut line = format!(
            "{}\t{}\t{pushed}\ttopics:{topics}",
            repo.full_name, repo.stargazers_count
        );
        if let Some(slug) = slug {
            line.push_str("\tlisted:");
            line.push_str(slug);
        }
        lines.push(line);
    }
    GithubReport {
        text: lines.join("\n"),
        warnings: found.warnings.clone(),
    }
}

#[cfg(test)]
mod tests {
    use catalogue_signals::github::FoundRepo;
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};
    use crate::discover::DiscoverCommand;

    fn found(name: &str, stars: u32, pushed: &str, topics: &[&str]) -> FoundRepo {
        FoundRepo {
            repo: Repo {
                full_name: name.to_owned(),
                html_url: format!("https://github.com/{name}"),
                description: Some("ignore\tme".to_owned()),
                stargazers_count: stars,
                pushed_at: pushed.to_owned(),
                created_at: String::new(),
                topics: Vec::new(),
                archived: false,
                fork: false,
                homepage: None,
            },
            matched_topics: topics.iter().map(|t| (*t).to_owned()).collect(),
        }
    }

    fn result(repos: Vec<FoundRepo>) -> GithubFound {
        GithubFound {
            repos,
            warnings: vec!["cap".to_owned()],
            requests: 3,
        }
    }

    #[test]
    fn leads_sort_by_stars_then_name_and_hide_the_description() {
        let all = result(vec![
            found("b/low", 20, "2026-02-03T04:05:06Z", &["llm"]),
            found("z/top", 90, "2026-01-02T00:00:00Z", &["llm", "inference"]),
            found("a/low", 20, "", &["llm"]),
        ]);
        let report = build_github_report(&all, &HashSet::new(), &HashMap::new());
        assert_eq!(
            report.text,
            "github leads: 3\n\
             z/top\t90\t2026-01-02\ttopics:llm,inference\n\
             a/low\t20\t\ttopics:llm\n\
             b/low\t20\t2026-02-03\ttopics:llm"
        );
        assert_eq!(report.warnings, ["cap"]);
    }

    #[test]
    fn repos_that_a_crate_links_to_are_dropped() {
        let all = result(vec![
            found("Owner/Repo", 50, "2026-01-01T00:00:00Z", &["llm"]),
            found("x/y", 5, "2026-01-01T00:00:00Z", &["llm"]),
        ]);
        let linked = HashSet::from(["github.com/owner/repo".to_owned()]);
        let report = build_github_report(&all, &linked, &HashMap::new());
        assert_eq!(
            report.text,
            "github leads: 1\nx/y\t5\t2026-01-01\ttopics:llm"
        );
    }

    #[test]
    fn listed_repos_name_their_entry() {
        let all = result(vec![found("x/y", 5, "2026-01-01T00:00:00Z", &["llm"])]);
        let listed = HashMap::from([("github.com/x/y".to_owned(), "yy".to_owned())]);
        let report = build_github_report(&all, &HashSet::new(), &listed);
        assert_eq!(
            report.text,
            "github leads: 1\nx/y\t5\t2026-01-01\ttopics:llm\tlisted:yy"
        );
    }

    #[test]
    fn no_leads_print_only_the_header() {
        let report = build_github_report(&GithubFound::default(), &HashSet::new(), &HashMap::new());
        assert_eq!(report.text, "github leads: 0");
    }

    #[test]
    fn discover_github_has_defaults_and_takes_options() {
        let cli = Cli::try_parse_from(["catalogue", "discover", "github"]).expect("parses");
        let Command::Discover {
            command: DiscoverCommand::Github { data, cache_dir },
        } = cli.command
        else {
            panic!("expected discover github");
        };
        assert_eq!((data, cache_dir), (PathBuf::from("data"), None));
        let cli = Cli::try_parse_from([
            "catalogue",
            "discover",
            "github",
            "--data",
            "d",
            "--cache-dir",
            "c",
        ])
        .expect("parses");
        let Command::Discover {
            command: DiscoverCommand::Github { data, cache_dir },
        } = cli.command
        else {
            panic!("expected discover github");
        };
        assert_eq!(
            (data, cache_dir),
            (PathBuf::from("d"), Some(PathBuf::from("c")))
        );
    }
}
