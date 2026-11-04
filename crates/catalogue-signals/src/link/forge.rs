//! A repo on a host other than GitHub that the manifest proof can read.

use std::fmt;

use url::Url;

/// A host other than GitHub that serves repo listings and raw files with no token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeHost {
    /// `codeberg.org`, which runs Forgejo.
    Codeberg,
    /// `gitlab.com`.
    Gitlab,
}

impl ForgeHost {
    /// The host name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Codeberg => "codeberg.org",
            Self::Gitlab => "gitlab.com",
        }
    }
}

/// A repo on a [`ForgeHost`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeRepo {
    /// The host.
    pub host: ForgeHost,
    /// The repo path on the host. `owner/name` on Codeberg, and any group depth on GitLab.
    pub path: String,
}

impl ForgeRepo {
    /// Reads a repo link.
    ///
    /// Takes an `http` or `https` link and keeps the case of the path. Drops a `.git` suffix and
    /// every path segment after the repo. Codeberg keeps two segments. GitLab keeps every
    /// segment before `-`, because a project sits under any depth of groups.
    ///
    /// Returns `None` for any other host, or a path with too few segments or an unsafe segment.
    pub fn parse(link: &str) -> Option<Self> {
        let url = Url::parse(link.trim()).ok()?;
        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }
        let name = url.host_str()?;
        let host = match name.strip_prefix("www.").unwrap_or(name) {
            "codeberg.org" => ForgeHost::Codeberg,
            "gitlab.com" => ForgeHost::Gitlab,
            _ => return None,
        };
        let segments = url.path_segments()?.filter(|segment| !segment.is_empty());
        let mut kept: Vec<&str> = match host {
            ForgeHost::Codeberg => segments.take(2).collect(),
            ForgeHost::Gitlab => segments.take_while(|segment| *segment != "-").collect(),
        };
        if kept.len() < 2 {
            return None;
        }
        if let Some(last) = kept.last_mut()
            && let Some(stem) = last.strip_suffix(".git")
        {
            *last = stem;
        }
        kept.iter()
            .all(|segment| is_safe_segment(segment))
            .then(|| Self {
                host,
                path: kept.join("/"),
            })
    }
}

impl fmt::Display for ForgeRepo {
    /// Writes `host/path`, the name of the repo in link results and in the cache.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.host.name(), self.path)
    }
}

/// Whether `segment` is a path segment that a URL and a file name carry as is.
fn is_safe_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(link: &str) -> String {
        ForgeRepo::parse(link).expect(link).path
    }

    #[test]
    fn codeberg_keeps_the_owner_and_the_name() {
        let repo = ForgeRepo::parse("https://codeberg.org/sarah-quinones/faer").expect("repo");
        assert_eq!(repo.host, ForgeHost::Codeberg);
        assert_eq!(repo.path, "sarah-quinones/faer");
        assert_eq!(repo.to_string(), "codeberg.org/sarah-quinones/faer");
        assert_eq!(path("https://codeberg.org/a_b/c.d.git/"), "a_b/c.d");
        assert_eq!(path("https://www.codeberg.org/a/b/src/branch/main"), "a/b");
    }

    #[test]
    fn gitlab_keeps_every_group_up_to_the_route_marker() {
        let repo = ForgeRepo::parse("https://gitlab.com/Group/Sub/Proj.git").expect("repo");
        assert_eq!(repo.host, ForgeHost::Gitlab);
        assert_eq!(repo.path, "Group/Sub/Proj");
        assert_eq!(path("https://gitlab.com/g/p/-/tree/main"), "g/p");
    }

    #[test]
    fn every_other_host_stays_out() {
        for link in [
            "https://github.com/a/b",
            "https://gitcode.com/a/b",
            "https://tulpa.dev/a/b",
            "https://codeberg.org.evil.io/a/b",
            "https://evil.io/codeberg.org/a/b",
        ] {
            assert_eq!(ForgeRepo::parse(link), None, "{link}");
        }
    }

    #[test]
    fn a_link_without_a_usable_path_stays_out() {
        for link in [
            "https://codeberg.org/a",
            "https://gitlab.com/g/-/p",
            "https://codeberg.org/a/b%20c",
            "https://codeberg.org/a/%2e%2e%2fb",
            "ssh://codeberg.org/a/b",
            "git@codeberg.org:a/b",
            "not a link",
        ] {
            assert_eq!(ForgeRepo::parse(link), None, "{link}");
        }
    }
}
