//! The one place that decides what each fetch error means for a run.
//!
//! | Error | Class |
//! | --- | --- |
//! | 401 from a source that sends the token | fatal |
//! | HTTP client start, invalid header, link cache IO, date range | fatal |
//! | 404, unusable crate name | definitive |
//! | network error, other status, 401 from a source with no token | fallback |
//! | broken body, bad JSON, bad `Link` header, rate limit retries exhausted | fallback |
//! | GraphQL error other than a missing repo, unreadable package | fallback |
//!
//! A missing repo in a GraphQL answer is no error here: `repo_facts` returns it
//! as [`RepoFacts::Gone`](crate::github::RepoFacts::Gone), a value like any other.

use std::fmt::Display;

use super::source::Source;
use crate::distribution::DistributionError;
use crate::github::GithubError;
use crate::http::HttpError;
use crate::link::LinkError;

/// What a fetch error means for the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// Every source is wrong, such as with a rejected token, or the machine
    /// itself is at fault. The run ends.
    Fatal,
    /// The source answered that the name does not exist. That answer is the
    /// signal, so no older value replaces it.
    Definitive,
    /// The source gave no answer for this entry. The previous value stands in.
    Fallback,
}

/// A fetch that ended without a value, when the run goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failed {
    /// A [`Failure::Definitive`] error, as text.
    Definitive(String),
    /// A [`Failure::Fallback`] error, as text.
    Fallback(String),
}

/// Sorts the error of `result` with `classify`.
///
/// # Errors
///
/// Returns the error itself when it is [`Failure::Fatal`].
pub fn attempt<T, E: Display>(
    result: Result<T, E>,
    classify: impl FnOnce(&E) -> Failure,
) -> Result<Result<T, Failed>, E> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(error) => match classify(&error) {
            Failure::Fatal => Err(error),
            Failure::Definitive => Ok(Err(Failed::Definitive(error.to_string()))),
            Failure::Fallback => Ok(Err(Failed::Fallback(error.to_string()))),
        },
    }
}

fn http_failure(error: &HttpError) -> Failure {
    match error {
        HttpError::Client { .. } | HttpError::InvalidHeader { .. } => Failure::Fatal,
        HttpError::Request { .. } => Failure::Fallback,
    }
}

/// The class of an error from a GitHub request, all of which send the token.
pub fn github_failure(error: &GithubError) -> Failure {
    match error {
        GithubError::Http(error) => http_failure(error),
        GithubError::Status { status: 401, .. } | GithubError::DateRange { .. } => Failure::Fatal,
        GithubError::Status { status: 404, .. } => Failure::Definitive,
        GithubError::Status { .. }
        | GithubError::Body { .. }
        | GithubError::Json { .. }
        | GithubError::RetriesExhausted { .. }
        | GithubError::BadLink { .. }
        | GithubError::Graphql { .. } => Failure::Fallback,
    }
}

/// The class of an error from the distribution lookup of `source`.
pub fn distribution_failure(error: &DistributionError, source: Source) -> Failure {
    match error {
        DistributionError::Http(error) => http_failure(error),
        DistributionError::Status { status: 401, .. } if source.sends_token() => Failure::Fatal,
        DistributionError::Status { status: 404, .. } => Failure::Definitive,
        DistributionError::Status { .. }
        | DistributionError::Body { .. }
        | DistributionError::Json { .. }
        | DistributionError::BadField { .. }
        | DistributionError::RetriesExhausted { .. } => Failure::Fallback,
    }
}

/// The class of an error from a repo link check.
pub fn link_failure(error: &LinkError) -> Failure {
    match error {
        LinkError::Http(error) => http_failure(error),
        LinkError::Github(error) => github_failure(error),
        LinkError::Download { status: 404, .. } | LinkError::UnsafeName { .. } => {
            Failure::Definitive
        }
        LinkError::Download { .. } | LinkError::Package { .. } => Failure::Fallback,
        LinkError::Cache { .. } => Failure::Fatal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn github(status: u16) -> GithubError {
        GithubError::Status {
            url: "u".to_owned(),
            status,
            action: "a",
        }
    }

    fn distribution(status: u16) -> DistributionError {
        DistributionError::Status {
            api: "x",
            url: "u".to_owned(),
            status,
            action: "a",
        }
    }

    fn request() -> HttpError {
        HttpError::Request {
            url: "u".to_owned(),
            source: "down".into(),
        }
    }

    fn client() -> HttpError {
        HttpError::Client {
            reason: "tls".to_owned(),
        }
    }

    #[test]
    fn github_statuses_sort_by_what_they_say() {
        assert_eq!(github_failure(&github(401)), Failure::Fatal);
        assert_eq!(github_failure(&github(404)), Failure::Definitive);
        for status in [403, 429, 500, 503] {
            assert_eq!(
                github_failure(&github(status)),
                Failure::Fallback,
                "{status}"
            );
        }
    }

    #[test]
    fn a_graphql_error_and_exhausted_retries_fall_back() {
        let graphql = GithubError::Graphql {
            repo: "a/b".to_owned(),
            message: "m".to_owned(),
        };
        assert_eq!(github_failure(&graphql), Failure::Fallback);
        let retries = GithubError::RetriesExhausted {
            url: "u".to_owned(),
            retries: 3,
        };
        assert_eq!(github_failure(&retries), Failure::Fallback);
    }

    #[test]
    fn a_network_error_falls_back_and_a_local_one_is_fatal() {
        assert_eq!(
            github_failure(&GithubError::Http(request())),
            Failure::Fallback
        );
        assert_eq!(github_failure(&GithubError::Http(client())), Failure::Fatal);
        let header = HttpError::InvalidHeader {
            name: "Authorization".to_owned(),
        };
        assert_eq!(github_failure(&GithubError::Http(header)), Failure::Fatal);
    }

    #[test]
    fn a_401_is_fatal_only_from_a_source_that_sends_the_token() {
        let error = distribution(401);
        assert_eq!(
            distribution_failure(&error, Source::Releases),
            Failure::Fatal
        );
        for source in [Source::Homebrew, Source::Repology, Source::Docker] {
            assert_eq!(distribution_failure(&error, source), Failure::Fallback);
        }
    }

    #[test]
    fn other_distribution_errors_fall_back() {
        for status in [403, 429, 500] {
            let error = distribution(status);
            assert_eq!(
                distribution_failure(&error, Source::Releases),
                Failure::Fallback,
                "{status}"
            );
        }
        assert_eq!(
            distribution_failure(&distribution(404), Source::Docker),
            Failure::Definitive
        );
        let network = DistributionError::Http(request());
        assert_eq!(
            distribution_failure(&network, Source::Docker),
            Failure::Fallback
        );
        let local = DistributionError::Http(client());
        assert_eq!(distribution_failure(&local, Source::Docker), Failure::Fatal);
    }

    #[test]
    fn link_errors_sort_by_their_cause() {
        let download = |status| LinkError::Download {
            url: "u".to_owned(),
            status,
        };
        assert_eq!(link_failure(&download(404)), Failure::Definitive);
        assert_eq!(link_failure(&download(500)), Failure::Fallback);
        assert_eq!(link_failure(&download(401)), Failure::Fallback);
        let unsafe_name = LinkError::UnsafeName {
            value: "a b".to_owned(),
        };
        assert_eq!(link_failure(&unsafe_name), Failure::Definitive);
        assert_eq!(
            link_failure(&LinkError::Github(github(401))),
            Failure::Fatal
        );
        assert_eq!(
            link_failure(&LinkError::Github(github(403))),
            Failure::Fallback
        );
        assert_eq!(link_failure(&LinkError::Http(request())), Failure::Fallback);
        let cache = LinkError::Cache {
            path: "p".into(),
            source: std::io::Error::other("full"),
        };
        assert_eq!(link_failure(&cache), Failure::Fatal);
    }

    #[test]
    fn attempt_ends_the_run_only_on_a_fatal_error() {
        let fatal = attempt::<u8, _>(Err("e"), |_| Failure::Fatal);
        assert_eq!(fatal, Err("e"));
        let definitive = attempt::<u8, _>(Err("e"), |_| Failure::Definitive);
        assert_eq!(definitive, Ok(Err(Failed::Definitive("e".to_owned()))));
        let fallback = attempt::<u8, _>(Err("e"), |_| Failure::Fallback);
        assert_eq!(fallback, Ok(Err(Failed::Fallback("e".to_owned()))));
        let found = attempt::<u8, &str>(Ok(1), |_| Failure::Fatal);
        assert_eq!(found, Ok(Ok(1)));
    }
}
