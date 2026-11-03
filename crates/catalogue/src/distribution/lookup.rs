//! The per-source lookups of `catalogue distribution`, each rendered as text.

use std::time::Duration;

use catalogue_signals::distribution::{
    DOCKER_INTERVAL, DistributionError, HOMEBREW_INTERVAL, RELEASES_INTERVAL, REPOLOGY_INTERVAL,
    docker_pulls, homebrew_installs, release_downloads, repology_packaging,
};
use catalogue_signals::github::RepoRef;
use catalogue_signals::http::{IntervalPacer, Pacer, ReqwestTransport, Transport};

use super::command::DistributionCommandError;
use super::render::{
    join_blocks, render_docker, render_homebrew, render_releases, render_repology,
};
use crate::batch;
use crate::token::NoToken;

/// Reads the releases of each repo and renders them.
pub(super) fn releases(
    arguments: &[String],
    token: Result<String, NoToken>,
) -> Result<String, DistributionCommandError> {
    let repos = arguments
        .iter()
        .map(|text| RepoRef::parse(text))
        .collect::<Result<Vec<_>, _>>()?;
    let token = token?;
    let blocks = each(&repos, RELEASES_INTERVAL, |transport, repo, pacer| {
        let found = release_downloads(transport, &token, repo, pacer)?;
        Ok(render_releases(&repo.to_string(), found.as_ref()))
    })?;
    Ok(join_blocks(&blocks))
}

/// Reads the install counts of each formula and renders them.
pub(super) fn homebrew(formulas: &[String]) -> Result<String, DistributionCommandError> {
    check_names(formulas)?;
    let blocks = each(formulas, HOMEBREW_INTERVAL, |transport, name, pacer| {
        let found = homebrew_installs(transport, name, pacer)?;
        Ok(render_homebrew(name, found.as_ref()))
    })?;
    Ok(join_blocks(&blocks))
}

/// Reads the pull count of each Docker Hub repo and renders it.
pub(super) fn docker(arguments: &[String]) -> Result<String, DistributionCommandError> {
    let repos = arguments
        .iter()
        .map(|text| split_docker_name(text).map(|(namespace, repo)| (text, namespace, repo)))
        .collect::<Result<Vec<_>, _>>()?;
    let blocks = each(&repos, DOCKER_INTERVAL, |transport, parts, pacer| {
        let (text, namespace, repo) = *parts;
        let found = docker_pulls(transport, namespace, repo, pacer)?;
        Ok(render_docker(text, found.as_ref()))
    })?;
    Ok(join_blocks(&blocks))
}

/// Reads the packaging of each Repology project and renders it.
pub(super) fn repology(projects: &[String]) -> Result<String, DistributionCommandError> {
    check_names(projects)?;
    let blocks = each(projects, REPOLOGY_INTERVAL, |transport, name, pacer| {
        let found = repology_packaging(transport, name, pacer)?;
        Ok(render_repology(name, found.as_ref()))
    })?;
    Ok(join_blocks(&blocks))
}

/// Looks up every item in order and returns the rendered blocks.
///
/// One pacer holds `interval` between two requests.
fn each<T>(
    items: &[T],
    interval: Duration,
    lookup: impl Fn(&dyn Transport, &T, &mut dyn Pacer) -> Result<String, DistributionError>,
) -> Result<Vec<String>, DistributionCommandError> {
    let transport = ReqwestTransport::new().map_err(DistributionError::from)?;
    let mut pacer = IntervalPacer::new(interval);
    Ok(batch::each(items, &mut pacer, |item, pacer| {
        lookup(&transport, item, pacer)
    })?)
}

/// Rejects a blank name.
fn check_names(names: &[String]) -> Result<(), DistributionCommandError> {
    if names.iter().any(|name| name.trim().is_empty()) {
        return Err(DistributionCommandError::EmptyName);
    }
    Ok(())
}

/// Splits `namespace/repo` into its two parts.
pub(crate) fn split_docker_name(input: &str) -> Result<(&str, &str), DistributionCommandError> {
    input
        .split_once('/')
        .filter(|(namespace, repo)| {
            [namespace, repo]
                .iter()
                .all(|part| !part.trim().is_empty() && !part.contains('/'))
        })
        .ok_or_else(|| DistributionCommandError::DockerName {
            input: input.to_owned(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exit::INVALID;

    #[test]
    fn a_bad_repo_exits_one_before_the_token_or_any_request() {
        let error =
            releases(&["a/b".to_owned(), "nope".to_owned()], Err(NoToken)).expect_err("bad repo");
        assert!(matches!(error, DistributionCommandError::Repo(_)));
        assert!(error.to_string().contains("`nope`"), "{error}");
        assert_eq!(error.exit_code(), INVALID);
    }

    #[test]
    fn a_missing_token_exits_one() {
        let error = releases(&["a/b".to_owned()], Err(NoToken)).expect_err("no token");
        assert!(matches!(error, DistributionCommandError::Token(_)));
        assert_eq!(error.exit_code(), INVALID);
    }

    #[test]
    fn docker_names_need_a_namespace_and_a_repo() {
        assert_eq!(
            split_docker_name("library/nginx").ok(),
            Some(("library", "nginx"))
        );
        for input in ["nginx", "/nginx", "library/", "a/b/c", " /b"] {
            assert!(split_docker_name(input).is_err(), "{input}");
        }
        let error = docker(&["nginx".to_owned()]).expect_err("bad name");
        assert!(error.to_string().contains("library/nginx"), "{error}");
        assert_eq!(error.exit_code(), INVALID);
    }

    #[test]
    fn a_blank_name_exits_one_before_any_request() {
        for error in [
            homebrew(&["ripgrep".to_owned(), " ".to_owned()]).expect_err("blank"),
            repology(&[String::new()]).expect_err("blank"),
        ] {
            assert!(matches!(error, DistributionCommandError::EmptyName));
            assert_eq!(error.exit_code(), INVALID);
        }
    }
}
