use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use url::Url;

use super::requirement::{exact_version, stays_below};
use crate::ids::{CrateName, canonical_crate_name};

/// Contents of `data/deprecated.toml`.
///
/// Requirements on crates that their own authors deprecated.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deprecated {
    /// The `[[requirement]]` tables.
    #[serde(default)]
    pub requirement: Vec<DeprecatedRequirement>,
}

/// One deprecated crate and the version below which its requirements are deprecated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeprecatedRequirement {
    /// The deprecated crate.
    #[serde(rename = "crate")]
    pub name: CrateName,
    /// A `major.minor.patch` version. A requirement that allows no version at
    /// or above it is deprecated.
    pub below: String,
    /// Why the author deprecated these versions.
    pub reason: String,
    /// The page where the crate's own authors or RustSec state the deprecation. Uses https.
    pub source: Url,
}

/// The outcome of checking one requirement against the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementCheck {
    /// The list names the crate and every version the requirement allows is below `below`.
    Deprecated,
    /// The list does not name the crate, or the requirement allows a version at or above `below`.
    Allowed,
    /// The list names the crate and the requirement holds a form the reader does not parse.
    Unparsed,
}

impl Deprecated {
    /// Lists every rule this file breaks.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for (index, item) in self.requirement.iter().enumerate() {
            let number = index + 1;
            if !seen.insert(item.name.normalized()) {
                out.push(format!(
                    "requirement {number}: crate `{}` is listed twice",
                    item.name
                ));
            }
            if exact_version(&item.below).is_none() {
                out.push(format!(
                    "requirement {number}: below `{}` must be a version such as `1.0.0`",
                    item.below
                ));
            }
            if item.reason.trim().is_empty() {
                out.push(format!("requirement {number}: reason must not be empty"));
            }
            if item.source.scheme() != "https" {
                out.push(format!(
                    "requirement {number}: source `{}` must use https",
                    item.source
                ));
            }
        }
        out
    }

    /// The entry for crate `name`, in any spelling crates.io treats as equal.
    pub fn find(&self, name: &str) -> Option<&DeprecatedRequirement> {
        let wanted = canonical_crate_name(name);
        self.requirement
            .iter()
            .find(|item| item.name.normalized() == wanted)
    }

    /// Checks the requirement `req` on crate `name`.
    pub fn check(&self, name: &str, req: &str) -> RequirementCheck {
        match self.find(name) {
            Some(item) => item.check(req),
            None => RequirementCheck::Allowed,
        }
    }
}

impl DeprecatedRequirement {
    /// Checks the requirement `req` on this crate. An invalid `below` reads as unparsed.
    pub fn check(&self, req: &str) -> RequirementCheck {
        let verdict = exact_version(&self.below).and_then(|below| stays_below(req, below));
        match verdict {
            Some(true) => RequirementCheck::Deprecated,
            Some(false) => RequirementCheck::Allowed,
            None => RequirementCheck::Unparsed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = r#"[[requirement]]
crate = "tokio"
below = "1.0.0"
reason = "Pre-1.0 releases."
source = "https://example.com/tokio"

[[requirement]]
crate = "rustc-serialize"
below = "999999.0.0"
reason = "Deprecated in favour of serde."
source = "https://example.com/rustc-serialize"
"#;

    fn parse(text: &str) -> Deprecated {
        toml::from_str(text).expect("parses")
    }

    #[test]
    fn valid_list_parses_and_passes() {
        let list = parse(LIST);
        assert_eq!(list.requirement.len(), 2);
        assert!(list.problems().is_empty(), "{:?}", list.problems());
        assert!(parse("").requirement.is_empty());
    }

    #[test]
    fn duplicate_names_bad_versions_and_empty_reasons_are_reported() {
        let extra = "crate = \"Rustc_Serialize\"\nbelow = \"1.0\"\nreason = \" \"\nsource = \"https://example.com/x\"\n";
        let text = format!("{LIST}\n[[requirement]]\n{extra}");
        let found = parse(&text).problems();
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found[0].contains("requirement 3: crate `Rustc_Serialize` is listed twice"));
        assert!(found[1].contains("below `1.0` must be a version"));
        assert!(found[2].contains("reason must not be empty"));
    }

    #[test]
    fn a_non_https_source_is_reported() {
        let text = LIST.replacen("https://example.com/tokio", "http://example.com/tokio", 1);
        let found = parse(&text).problems();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("requirement 1: source `http://example.com/tokio` must use https")
        );
    }

    #[test]
    fn a_missing_or_malformed_source_fails_to_parse() {
        let missing = LIST.replacen("source = \"https://example.com/tokio\"\n", "", 1);
        assert!(toml::from_str::<Deprecated>(&missing).is_err());
        let malformed = LIST.replacen("https://example.com/tokio", "not a url", 1);
        assert!(toml::from_str::<Deprecated>(&malformed).is_err());
    }

    #[test]
    fn unknown_fields_fail_to_parse() {
        let text = LIST.replace("below = \"1.0.0\"", "below = \"1.0.0\"\nextra = 1");
        assert!(toml::from_str::<Deprecated>(&text).is_err());
    }

    #[test]
    fn check_reads_the_list_by_crates_io_name() {
        let list = parse(LIST);
        assert_eq!(list.check("tokio", "0.1"), RequirementCheck::Deprecated);
        assert_eq!(list.check("Tokio", "^0.2.4"), RequirementCheck::Deprecated);
        assert_eq!(list.check("tokio", "1"), RequirementCheck::Allowed);
        assert_eq!(list.check("tokio", "*"), RequirementCheck::Allowed);
        assert_eq!(list.check("tokio", "~>1"), RequirementCheck::Unparsed);
        assert_eq!(list.check("serde", "0.1"), RequirementCheck::Allowed);
        assert_eq!(list.check("serde", "~>1"), RequirementCheck::Allowed);
        assert_eq!(
            list.check("rustc_serialize", "0.3.25"),
            RequirementCheck::Deprecated
        );
        assert!(list.find("RUSTC-SERIALIZE").is_some());
    }

    #[test]
    fn an_invalid_below_reads_as_unparsed() {
        let item = DeprecatedRequirement {
            name: CrateName::new("x").expect("name"),
            below: "1".to_owned(),
            reason: "r".to_owned(),
            source: Url::parse("https://example.com/x").expect("url"),
        };
        assert_eq!(item.check("0.1"), RequirementCheck::Unparsed);
    }
}
