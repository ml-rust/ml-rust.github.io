use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::ids::GithubLogin;

/// Contents of `data/maintainers.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Maintainers {
    /// The `[[maintainer]]` tables.
    #[serde(default)]
    pub maintainer: Vec<Maintainer>,
}

/// A person who can approve judgments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Maintainer {
    /// GitHub login.
    pub login: GithubLogin,
    /// Whether the person belongs to the ml-rust organization.
    pub org_member: bool,
    /// Date the person became a maintainer.
    pub since: Date,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_maintainer_tables() {
        let parsed: Maintainers = toml::from_str(
            "[[maintainer]]\nlogin = \"carol\"\norg_member = true\nsince = 2026-01-01\n",
        )
        .expect("parses");
        assert_eq!(parsed.maintainer.len(), 1);
        assert!(parsed.maintainer.iter().all(|m| m.org_member));
    }

    #[test]
    fn empty_file_is_an_empty_list() {
        let parsed: Maintainers = toml::from_str("").expect("parses");
        assert!(parsed.maintainer.is_empty());
    }

    #[test]
    fn rejects_bad_login_and_unknown_field() {
        let bad = "[[maintainer]]\nlogin = \"-x\"\norg_member = true\nsince = 2026-01-01\n";
        assert!(toml::from_str::<Maintainers>(bad).is_err());
        let extra =
            "[[maintainer]]\nlogin = \"x\"\norg_member = true\nsince = 2026-01-01\nrole = 1\n";
        assert!(toml::from_str::<Maintainers>(extra).is_err());
    }
}
