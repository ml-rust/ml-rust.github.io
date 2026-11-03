//! The text report of `catalogue ecosystems crates`.

use catalogue_signals::ecosystems::CrateDependents;

/// The value printed for a crate that ecosyste.ms does not know.
const NONE: &str = "-";

/// The `key: value` lines of one crate, with no trailing newline.
///
/// The keys are `crate`, `dependent_packages`, `dependent_repos`,
/// `docker_dependents` and `docker_downloads`. `crate` holds the requested
/// name. A crate that ecosyste.ms does not know prints `-` for every other key.
/// A `fields_missing` line follows when the API left a count out. It lists the
/// API field names, joined with `,`.
pub fn render_crate(name: &str, found: Option<&CrateDependents>) -> String {
    let mut lines = vec![format!("crate: {name}")];
    match found {
        Some(found) => {
            lines.push(format!("dependent_packages: {}", found.dependent_packages));
            lines.push(format!("dependent_repos: {}", found.dependent_repos));
            lines.push(format!("docker_dependents: {}", found.docker_dependents));
            lines.push(format!("docker_downloads: {}", found.docker_downloads));
            if !found.fields_missing.is_empty() {
                lines.push(format!(
                    "fields_missing: {}",
                    found.fields_missing.join(",")
                ));
            }
        }
        None => {
            for key in [
                "dependent_packages",
                "dependent_repos",
                "docker_dependents",
                "docker_downloads",
            ] {
                lines.push(format!("{key}: {NONE}"));
            }
        }
    }
    lines.join("\n")
}

/// Renders every crate, with a blank line between crates.
pub fn render_all(results: &[(String, Option<CrateDependents>)]) -> String {
    results
        .iter()
        .map(|(name, found)| render_crate(name, found.as_ref()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(fields_missing: Vec<&'static str>) -> CrateDependents {
        CrateDependents {
            dependent_packages: 47,
            dependent_repos: 1,
            docker_dependents: 9,
            docker_downloads: 18_669_296,
            fields_missing,
        }
    }

    #[test]
    fn a_found_crate_prints_every_count() {
        assert_eq!(
            render_crate("candle-core", Some(&counts(Vec::new()))),
            "crate: candle-core\ndependent_packages: 47\ndependent_repos: 1\n\
             docker_dependents: 9\ndocker_downloads: 18669296"
        );
    }

    #[test]
    fn missing_fields_are_listed() {
        let text = render_crate(
            "x",
            Some(&counts(vec![
                "dependent_repos_count",
                "docker_downloads_count",
            ])),
        );
        assert!(
            text.ends_with("\nfields_missing: dependent_repos_count,docker_downloads_count"),
            "{text}"
        );
    }

    #[test]
    fn an_unknown_crate_prints_dashes() {
        assert_eq!(
            render_crate("nope", None),
            "crate: nope\ndependent_packages: -\ndependent_repos: -\n\
             docker_dependents: -\ndocker_downloads: -"
        );
    }

    #[test]
    fn crates_are_separated_by_a_blank_line() {
        let text = render_all(&[
            ("a".to_owned(), Some(counts(Vec::new()))),
            ("b".to_owned(), None),
        ]);
        let blocks: Vec<&str> = text.split("\n\n").collect();
        assert_eq!(blocks.len(), 2);
        assert!(blocks[1].starts_with("crate: b\n"));
        assert_eq!(render_all(&[]), "");
    }
}
