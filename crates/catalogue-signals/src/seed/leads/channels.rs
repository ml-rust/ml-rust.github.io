//! The install channels of an app draft.
//!
//! Each channel reads an existing distribution fetcher under one fixed name. An unmatched channel
//! stays empty. A failed lookup also leaves it empty and adds a warning.

use catalogue_core::{Distribution, normalize_repo_url};

use super::types::LeadPacers;
use crate::distribution::{docker_pulls, homebrew_source_links, repology_packaging};
use crate::github::RepoRef;
use crate::http::Transport;

/// The channels of one app, with the labels and warnings for the reviewer.
pub(super) struct Channels {
    pub(super) distribution: Option<Distribution>,
    pub(super) notes: Vec<String>,
    pub(super) warnings: Vec<String>,
}

/// Looks up the channels of `canonical`. `keys` hold the normalised names of the repository.
///
/// - Homebrew: the formula named like the repository, when one of its source links names the
///   repository. A formula with no such link stays out.
/// - Repology: the project named like the repository. Repology exposes no source link, so the
///   label says the match is by name only.
/// - Docker Hub: `owner/name` under the GitHub owner.
pub(super) fn lookup_channels(
    transport: &dyn Transport,
    canonical: &RepoRef,
    keys: &[String],
    pacers: &mut LeadPacers<'_>,
) -> Channels {
    let owner = canonical.owner.to_ascii_lowercase();
    let name = canonical.name.to_ascii_lowercase();
    let mut notes = Vec::new();
    let mut warnings = Vec::new();
    let mut failed = |channel: &str, error: String| {
        warnings.push(format!(
            "{canonical}: the {channel} lookup failed and the channel stays empty: {error}"
        ));
        format!("{channel}:lookup-failed")
    };

    let homebrew = match homebrew_source_links(transport, &name, &mut *pacers.homebrew) {
        Ok(Some(links)) if names_repository(&links, keys) => {
            notes.push(format!("homebrew:{name}"));
            Some(name.clone())
        }
        Ok(Some(_)) => {
            notes.push(format!("homebrew-unmatched:{name}"));
            None
        }
        Ok(None) => None,
        Err(error) => {
            notes.push(failed("homebrew", error.to_string()));
            None
        }
    };
    let repology = match repology_packaging(transport, &name, &mut *pacers.repology) {
        Ok(Some(_)) => {
            notes.push(format!("repology:{name}:name-only"));
            Some(name.clone())
        }
        Ok(None) => None,
        Err(error) => {
            notes.push(failed("repology", error.to_string()));
            None
        }
    };
    let docker = match docker_pulls(transport, &owner, &name, &mut *pacers.docker) {
        Ok(Some(_)) => {
            notes.push(format!("docker:{owner}/{name}"));
            vec![format!("{owner}/{name}")]
        }
        Ok(None) => Vec::new(),
        Err(error) => {
            notes.push(failed("docker", error.to_string()));
            Vec::new()
        }
    };
    let empty = homebrew.is_none() && repology.is_none() && docker.is_empty();
    Channels {
        distribution: (!empty).then_some(Distribution {
            homebrew,
            docker,
            repology,
        }),
        notes,
        warnings,
    }
}

fn names_repository(links: &[String], keys: &[String]) -> bool {
    links
        .iter()
        .filter_map(|link| normalize_repo_url(link))
        .any(|key| keys.contains(&key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> [String; 2] {
        [
            "github.com/new/tool".to_owned(),
            "github.com/old/tool".to_owned(),
        ]
    }

    fn named(links: &[&str]) -> bool {
        let links: Vec<String> = links.iter().map(|link| (*link).to_owned()).collect();
        names_repository(&links, &keys())
    }

    #[test]
    fn a_source_link_must_name_the_repository() {
        assert!(named(&["https://github.com/New/Tool/archive/v1.tar.gz"]));
        assert!(named(&["https://github.com/old/tool.git"]));
        assert!(!named(&["https://github.com/other/tool.git"]));
        assert!(!named(&["https://tool.example"]));
        assert!(!named(&[]));
    }
}
