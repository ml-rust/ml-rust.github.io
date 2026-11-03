//! The text reports of `catalogue distribution`.

use catalogue_signals::distribution::{
    DockerPulls, HomebrewInstalls, ReleaseDownloads, RepologyPackaging,
};

use crate::render::{NONE, or_none};

/// Joins the blocks of several names with a blank line between them.
pub(super) fn join_blocks(blocks: &[String]) -> String {
    blocks.join("\n\n")
}

/// The `key: value` lines of one repo's releases, with no trailing newline.
///
/// The keys are `repo`, `releases`, `total_binary_downloads` and one `release`
/// line per release. A `release` line reads
/// `<tag> published=<day or -> prerelease=<bool> binary_downloads=<n>`. `repo`
/// holds the requested name. A repo that GitHub does not know prints `-` for
/// `releases` and `total_binary_downloads`.
pub fn render_releases(name: &str, found: Option<&ReleaseDownloads>) -> String {
    let mut lines = vec![format!("repo: {name}")];
    match found {
        Some(found) => {
            lines.push(format!("releases: {}", found.releases.len()));
            lines.push(format!(
                "total_binary_downloads: {}",
                found.total_binary_downloads
            ));
            for release in &found.releases {
                let published = or_none(release.published);
                lines.push(format!(
                    "release: {} published={published} prerelease={} binary_downloads={}",
                    release.tag, release.prerelease, release.binary_downloads
                ));
            }
        }
        None => {
            lines.push(format!("releases: {NONE}"));
            lines.push(format!("total_binary_downloads: {NONE}"));
        }
    }
    lines.join("\n")
}

/// The `key: value` lines of one formula, with no trailing newline.
///
/// The keys are `formula`, `install_30d`, `install_90d`, `install_365d`,
/// `on_request_30d`, `on_request_90d` and `on_request_365d`. A formula that
/// Homebrew does not know prints `-` for every count.
pub fn render_homebrew(name: &str, found: Option<&HomebrewInstalls>) -> String {
    let counts = found.map(|found| {
        [
            found.install_30d,
            found.install_90d,
            found.install_365d,
            found.on_request_30d,
            found.on_request_90d,
            found.on_request_365d,
        ]
    });
    let keys = [
        "install_30d",
        "install_90d",
        "install_365d",
        "on_request_30d",
        "on_request_90d",
        "on_request_365d",
    ];
    let mut lines = vec![format!("formula: {name}")];
    for (index, key) in keys.iter().enumerate() {
        let value = or_none(counts.and_then(|counts| counts.get(index).copied()));
        lines.push(format!("{key}: {value}"));
    }
    lines.join("\n")
}

/// The `key: value` lines of one Docker Hub repo, with no trailing newline.
///
/// The keys are `repo`, `pull_count` and `last_updated`. `last_updated` is an
/// RFC 3339 UTC timestamp, or `-` when Docker Hub omits it. A repo that Docker
/// Hub does not know prints `-` for both.
pub fn render_docker(name: &str, found: Option<&DockerPulls>) -> String {
    let pulls = or_none(found.map(|found| found.pull_count));
    let updated = or_none(found.and_then(|found| found.last_updated));
    format!("repo: {name}\npull_count: {pulls}\nlast_updated: {updated}")
}

/// The `key: value` lines of one Repology project, with no trailing newline.
///
/// The keys are `project`, `families` and `repos`. `repos` joins the repo
/// names with `,`. A project that Repology does not know prints `-` for both.
pub fn render_repology(name: &str, found: Option<&RepologyPackaging>) -> String {
    let families = or_none(found.map(|found| found.families));
    let repos = or_none(found.map(|found| found.repos.join(",")));
    format!("project: {name}\nfamilies: {families}\nrepos: {repos}")
}

#[cfg(test)]
mod tests {
    use catalogue_core::Date;
    use catalogue_signals::distribution::ReleaseSummary;

    use super::*;

    fn downloads() -> ReleaseDownloads {
        ReleaseDownloads {
            releases: vec![
                ReleaseSummary {
                    tag: "15.2.0".to_owned(),
                    published: Date::new(2026, 7, 15).ok(),
                    prerelease: false,
                    binary_downloads: 5_580_009,
                },
                ReleaseSummary {
                    tag: "15.1.0-rc".to_owned(),
                    published: None,
                    prerelease: true,
                    binary_downloads: 4,
                },
            ],
            total_binary_downloads: 5_580_013,
        }
    }

    #[test]
    fn releases_print_a_total_and_one_line_per_release() {
        assert_eq!(
            render_releases("BurntSushi/ripgrep", Some(&downloads())),
            "repo: BurntSushi/ripgrep\nreleases: 2\ntotal_binary_downloads: 5580013\n\
             release: 15.2.0 published=2026-07-15 prerelease=false binary_downloads=5580009\n\
             release: 15.1.0-rc published=- prerelease=true binary_downloads=4"
        );
    }

    #[test]
    fn an_unknown_repo_prints_dashes() {
        assert_eq!(
            render_releases("a/b", None),
            "repo: a/b\nreleases: -\ntotal_binary_downloads: -"
        );
    }

    #[test]
    fn homebrew_prints_six_counts_or_dashes() {
        let found = HomebrewInstalls {
            install_30d: 1,
            install_90d: 2,
            install_365d: 3,
            on_request_30d: 4,
            on_request_90d: 5,
            on_request_365d: 6,
        };
        assert_eq!(
            render_homebrew("ripgrep", Some(&found)),
            "formula: ripgrep\ninstall_30d: 1\ninstall_90d: 2\ninstall_365d: 3\n\
             on_request_30d: 4\non_request_90d: 5\non_request_365d: 6"
        );
        let none = render_homebrew("nope", None);
        assert_eq!(none.matches(": -").count(), 6, "{none}");
    }

    #[test]
    fn docker_prints_the_time_or_a_dash() {
        let when = "2026-11-03T04:52:35Z".parse().ok();
        let found = DockerPulls {
            pull_count: 12,
            last_updated: when,
        };
        assert_eq!(
            render_docker("library/nginx", Some(&found)),
            "repo: library/nginx\npull_count: 12\nlast_updated: 2026-11-03T04:52:35Z"
        );
        let missing = DockerPulls {
            last_updated: None,
            ..found
        };
        assert!(render_docker("x/y", Some(&missing)).ends_with("last_updated: -"));
        assert_eq!(
            render_docker("x/y", None),
            "repo: x/y\npull_count: -\nlast_updated: -"
        );
    }

    #[test]
    fn repology_joins_repos() {
        let found = RepologyPackaging {
            repos: vec!["debian_12".to_owned(), "homebrew".to_owned()],
            families: 2,
        };
        assert_eq!(
            render_repology("ripgrep", Some(&found)),
            "project: ripgrep\nfamilies: 2\nrepos: debian_12,homebrew"
        );
        assert_eq!(
            render_repology("nope", None),
            "project: nope\nfamilies: -\nrepos: -"
        );
    }

    #[test]
    fn blocks_are_separated_by_a_blank_line() {
        let text = join_blocks(&[render_docker("a/b", None), render_docker("c/d", None)]);
        assert_eq!(text.split("\n\n").count(), 2);
        assert_eq!(join_blocks(&[]), "");
    }
}
