//! The network sources of one entry.

use std::fmt;

/// One network source of an entry's signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// The GitHub repo facts.
    RepoFacts,
    /// The repo link check.
    RepoLink,
    /// The GitHub release counts.
    Releases,
    /// The Homebrew install counts.
    Homebrew,
    /// The Repology packaging count.
    Repology,
    /// The Docker Hub pull count.
    Docker,
}

impl Source {
    /// The name in notes and in the run summary.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RepoFacts => "GitHub repo facts",
            Self::RepoLink => "repo link check",
            Self::Releases => "GitHub releases",
            Self::Homebrew => "Homebrew",
            Self::Repology => "Repology",
            Self::Docker => "Docker Hub",
        }
    }

    /// Whether requests to this source send the GitHub token.
    pub fn sends_token(self) -> bool {
        matches!(self, Self::RepoFacts | Self::RepoLink | Self::Releases)
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
