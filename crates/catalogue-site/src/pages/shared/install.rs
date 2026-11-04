//! How to get a project, from the data its entry holds.

use catalogue_core::{Entry, EntryKind, crate_page, docker_page, homebrew_page, repology_page};

use crate::href::SafeHref;

/// The site that hosts the docs of every crate.
const DOCS_HOST: &str = "https://docs.rs/";

/// The docs of `entry`: docs.rs for a library's primary crate. `None` for any other entry.
pub(crate) fn docs_href(entry: &Entry) -> Option<SafeHref> {
    entry
        .crates
        .first()
        .filter(|_| entry.kind == EntryKind::Library)
        .map(|name| SafeHref::new(&format!("{DOCS_HOST}{name}")))
}

/// One way to get a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallLine {
    /// The command to run. `None` when the channel has no one command.
    pub command: Option<String>,
    /// The text of the link.
    pub text: &'static str,
    /// The page of the channel, or the repository.
    pub href: SafeHref,
}

impl InstallLine {
    fn new(command: Option<String>, text: &'static str, url: &url::Url) -> Self {
        Self {
            command,
            text,
            href: SafeHref::from(url),
        }
    }
}

/// How to get `entry`, using only the data it holds.
///
/// A library gets `cargo add` with its primary crate, the first it lists. An app gets its
/// Homebrew formula, else its first Docker image, else its Repology project, else `cargo install`
/// with its primary crate. Every other entry gets the repository link.
pub(crate) fn install_line(entry: &Entry) -> InstallLine {
    let channel = match entry.kind {
        EntryKind::Library => crate_line(entry, "add"),
        EntryKind::App => channel_line(entry).or_else(|| crate_line(entry, "install")),
    };
    channel.unwrap_or_else(|| InstallLine::new(None, "Repository", &entry.repo))
}

/// `cargo <verb>` with the primary crate of `entry`.
fn crate_line(entry: &Entry, verb: &str) -> Option<InstallLine> {
    let name = entry.crates.first()?;
    let url = crate_page(name.as_str())?;
    let command = format!("cargo {verb} {name}");
    Some(InstallLine::new(Some(command), "crates.io", &url))
}

/// The first usable channel that `entry` names.
fn channel_line(entry: &Entry) -> Option<InstallLine> {
    let channels = entry.distribution.as_ref()?;
    let homebrew = channels
        .homebrew
        .as_deref()
        .filter(|formula| is_command_word(formula))
        .and_then(|formula| {
            let command = format!("brew install {formula}");
            let url = homebrew_page(formula)?;
            Some(InstallLine::new(Some(command), "Homebrew", &url))
        });
    let docker = || {
        let image = channels
            .docker
            .iter()
            .find(|image| is_command_word(image))?;
        let command = format!("docker pull {image}");
        let url = docker_page(image)?;
        Some(InstallLine::new(Some(command), "Docker Hub", &url))
    };
    let repology = || {
        let project = channels
            .repology
            .as_deref()
            .filter(|project| is_command_word(project))?;
        let url = repology_page(project)?;
        Some(InstallLine::new(None, "Repology", &url))
    };
    homebrew.or_else(docker).or_else(repology)
}

/// Whether a person can paste `value` into a shell as one plain argument.
///
/// Channel names come from data files, and a reader copies the command. A name with any other
/// character gives no line.
fn is_command_word(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/' | b'@' | b'+' | b':')
        })
}

#[cfg(test)]
mod tests {
    use catalogue_core::{CrateName, Date, Description, DescriptionSource, Distribution, Slug};
    use url::Url;

    use super::*;

    const REPO: &str = "https://github.com/o/tool";

    fn entry(kind: EntryKind, crates: &[&str], distribution: Option<Distribution>) -> Entry {
        Entry {
            name: "tool".to_owned(),
            kind,
            repo: Url::parse(REPO).expect("repo"),
            homepage: None,
            crates: crates
                .iter()
                .map(|name| CrateName::new(*name).expect("crate name"))
                .collect(),
            topics: vec![Slug::new("topic").expect("slug")],
            category: None,
            part_of: None,
            description: Description {
                text: "A tool.".to_owned(),
                source: DescriptionSource::Human,
                source_url: None,
            },
            org_affiliated: false,
            distribution,
            added: Date::new(2026, 11, 3).expect("date"),
        }
    }

    fn channels(homebrew: Option<&str>, docker: &[&str], repology: Option<&str>) -> Distribution {
        Distribution {
            homebrew: homebrew.map(str::to_owned),
            docker: docker.iter().map(|image| (*image).to_owned()).collect(),
            repology: repology.map(str::to_owned),
        }
    }

    fn app(distribution: Distribution) -> InstallLine {
        install_line(&entry(EntryKind::App, &[], Some(distribution)))
    }

    fn command(line: &InstallLine) -> Option<&str> {
        line.command.as_deref()
    }

    #[test]
    fn a_library_adds_its_primary_crate() {
        let line = install_line(&entry(EntryKind::Library, &["tool-core", "tool-nn"], None));
        assert_eq!(command(&line), Some("cargo add tool-core"));
        assert_eq!(line.text, "crates.io");
        assert_eq!(line.href.as_str(), "https://crates.io/crates/tool-core");
    }

    #[test]
    fn an_app_prefers_homebrew_then_docker_then_repology() {
        let every = app(channels(Some("tool"), &["o/tool", "o/other"], Some("tool")));
        assert_eq!(command(&every), Some("brew install tool"));
        assert_eq!(every.href.as_str(), "https://formulae.brew.sh/formula/tool");

        let docker = app(channels(None, &["o/tool", "o/other"], Some("tool")));
        assert_eq!(command(&docker), Some("docker pull o/tool"));
        assert_eq!(docker.href.as_str(), "https://hub.docker.com/r/o/tool");

        let repology = app(channels(None, &[], Some("tool")));
        assert_eq!(command(&repology), None);
        assert_eq!(repology.text, "Repology");
        assert_eq!(
            repology.href.as_str(),
            "https://repology.org/project/tool/versions"
        );
    }

    #[test]
    fn an_app_without_a_channel_links_its_repository() {
        for line in [
            install_line(&entry(EntryKind::App, &[], None)),
            app(channels(None, &[], None)),
        ] {
            assert_eq!(command(&line), None);
            assert_eq!(line.text, "Repository");
            assert_eq!(line.href.as_str(), REPO);
        }
    }

    #[test]
    fn an_app_with_crates_and_no_channel_installs_its_primary_crate() {
        for line in [
            install_line(&entry(EntryKind::App, &["tool", "tool-core"], None)),
            install_line(&entry(
                EntryKind::App,
                &["tool", "tool-core"],
                Some(channels(None, &[], None)),
            )),
            install_line(&entry(
                EntryKind::App,
                &["tool", "tool-core"],
                Some(channels(Some("a b"), &[], None)),
            )),
        ] {
            assert_eq!(command(&line), Some("cargo install tool"));
            assert_eq!(line.text, "crates.io");
            assert_eq!(line.href.as_str(), "https://crates.io/crates/tool");
        }
    }

    #[test]
    fn the_channels_an_app_names_come_before_its_crate() {
        let on = |distribution| install_line(&entry(EntryKind::App, &["tool"], Some(distribution)));
        let homebrew = on(channels(Some("tool"), &[], None));
        assert_eq!(command(&homebrew), Some("brew install tool"));
        let docker = on(channels(None, &["o/tool"], None));
        assert_eq!(command(&docker), Some("docker pull o/tool"));
        let repology = on(channels(None, &[], Some("tool")));
        assert_eq!(command(&repology), None);
        assert_eq!(repology.text, "Repology");
    }

    #[test]
    fn a_name_that_is_not_one_plain_argument_gives_no_command() {
        for bad in ["", "tool; curl x | sh", "$(x)", "--help", "<b>", "a b"] {
            let line = app(channels(Some(bad), &[bad], Some(bad)));
            assert_eq!(command(&line), None, "{bad:?}");
            assert_eq!(line.text, "Repository", "{bad:?}");
        }
        let line = app(channels(Some("a b"), &["o/tool"], None));
        assert_eq!(command(&line), Some("docker pull o/tool"));
    }
}
