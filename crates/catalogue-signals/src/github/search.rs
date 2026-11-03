//! The topic search with date-range splitting.

use std::collections::BTreeMap;

use jiff::civil::{self, Date};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use serde::Deserialize;

use super::error::GithubError;
use super::json::parse_json;
use super::query::{PER_PAGE, search_url};
use super::repo::{FoundRepo, GithubFound, GithubRules, Repo};
use crate::http::{Paced, Pacer, Transport};

/// The most repos one query serves.
const MAX_RESULTS: u64 = 1000;
/// The pages that hold [`MAX_RESULTS`] repos.
const MAX_PAGES: u32 = 10;
/// The day before which GitHub holds no repos.
const FIRST_DAY: Date = civil::date(2008, 1, 1);

/// Searches every topic in `rules`, with repos created up to today (UTC).
///
/// `token` is sent as a bearer token. `pacing` holds each request back until
/// the API allows it.
///
/// # Errors
///
/// Returns a [`GithubError`] when a request fails, the server answers with an
/// error status, a body is not the expected JSON, or the rate limit does not
/// lift after the retries.
pub fn search_repos(
    transport: &dyn Transport,
    token: &str,
    rules: &GithubRules,
    pacing: &mut dyn Pacer,
) -> Result<GithubFound, GithubError> {
    let today = Timestamp::now().to_zoned(TimeZone::UTC).date();
    search_repos_on(transport, token, rules, pacing, today)
}

/// Searches like [`search_repos`], with repos created up to `today`.
///
/// # Errors
///
/// Returns a [`GithubError`] as [`search_repos`] does.
pub fn search_repos_on(
    transport: &dyn Transport,
    token: &str,
    rules: &GithubRules,
    pacing: &mut dyn Pacer,
    today: Date,
) -> Result<GithubFound, GithubError> {
    let mut run = Run {
        transport,
        authorization: format!("Bearer {token}"),
        paced: Paced::new(pacing),
        min_stars: rules.min_stars,
        found: BTreeMap::new(),
        warnings: Vec::new(),
    };
    for topic in &rules.topics {
        let mut repos = Vec::new();
        run.range(topic, FIRST_DAY, today, &mut repos)?;
        for repo in repos {
            run.add(topic, repo);
        }
    }
    Ok(GithubFound {
        repos: run.found.into_values().collect(),
        warnings: run.warnings,
        requests: run.paced.requests(),
    })
}

/// One page of search results.
#[derive(Deserialize)]
struct Page {
    total_count: u64,
    items: Vec<Repo>,
}

/// The state of one search.
struct Run<'a> {
    transport: &'a dyn Transport,
    authorization: String,
    paced: Paced<'a>,
    min_stars: u32,
    found: BTreeMap<String, FoundRepo>,
    warnings: Vec<String>,
}

impl Run<'_> {
    /// Collects the repos of `topic` created from `from` to `to` inclusive.
    fn range(
        &mut self,
        topic: &str,
        from: Date,
        to: Date,
        out: &mut Vec<Repo>,
    ) -> Result<(), GithubError> {
        let first = self.page(&search_url(topic, self.min_stars, from, to, 1))?;
        if first.total_count > MAX_RESULTS {
            if from == to {
                self.warnings.push(format!(
                    "topic `{topic}` has {} repos created on {from}; kept the first {MAX_RESULTS}",
                    first.total_count
                ));
            } else {
                let (left_end, right_start) = split(from, to)?;
                self.range(topic, from, left_end, out)?;
                return self.range(topic, right_start, to, out);
            }
        }
        let total = first.total_count.min(MAX_RESULTS);
        let mut last_len = first.items.len();
        let mut collected = first.items;
        let mut number = 1;
        while last_len == PER_PAGE && as_u64(collected.len()) < total && number < MAX_PAGES {
            number += 1;
            let next = self.page(&search_url(topic, self.min_stars, from, to, number))?;
            last_len = next.items.len();
            collected.extend(next.items);
        }
        out.extend(collected);
        Ok(())
    }

    /// Records `repo` as found by `topic`, merging with an earlier find.
    fn add(&mut self, topic: &str, repo: Repo) {
        let entry = self
            .found
            .entry(repo.full_name.to_lowercase())
            .or_insert_with(|| FoundRepo {
                repo,
                matched_topics: Vec::new(),
            });
        if !entry.matched_topics.iter().any(|t| t == topic) {
            entry.matched_topics.push(topic.to_owned());
        }
    }

    /// Fetches and parses one page, retrying rate-limit answers.
    fn page(&mut self, url: &str) -> Result<Page, GithubError> {
        let transport = self.transport;
        let headers = [
            ("Accept", "application/vnd.github+json".to_owned()),
            ("X-GitHub-Api-Version", "2022-11-28".to_owned()),
            ("Authorization", self.authorization.clone()),
        ];
        let bytes = self.paced.fetch(url, || transport.get(url, &headers))?;
        parse_json(url, &bytes)
    }
}

/// Halves `from..=to` into the end of the left half and the start of the right.
fn split(from: Date, to: Date) -> Result<(Date, Date), GithubError> {
    let error = |reason: jiff::Error| GithubError::DateRange {
        from: from.to_string(),
        to: to.to_string(),
        reason: reason.to_string(),
    };
    let days = from.until(to).map_err(error)?.get_days();
    let left_end = from
        .checked_add(Span::new().days(days / 2))
        .map_err(error)?;
    let right_start = left_end.checked_add(Span::new().days(1)).map_err(error)?;
    Ok((left_end, right_start))
}

fn as_u64(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    #[test]
    fn a_range_splits_in_halves_that_cover_it() {
        let (left, right) = split(date(2008, 1, 1), date(2008, 1, 3)).expect("splits");
        assert_eq!((left, right), (date(2008, 1, 2), date(2008, 1, 3)));
        let (left, right) = split(date(2008, 1, 1), date(2008, 1, 2)).expect("splits");
        assert_eq!((left, right), (date(2008, 1, 1), date(2008, 1, 2)));
        let (left, right) = split(date(2008, 1, 1), date(2008, 1, 6)).expect("splits");
        assert_eq!((left, right), (date(2008, 1, 3), date(2008, 1, 4)));
    }
}
