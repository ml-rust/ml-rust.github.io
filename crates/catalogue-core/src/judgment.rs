use serde::{Deserialize, Serialize};
use url::Url;

use crate::date::Date;
use crate::ids::{GithubLogin, Slug};
use crate::tier::Tier;

/// Prefix of every judgment issue link.
pub const ISSUE_URL_PREFIX: &str = "https://github.com/ml-rust/ml-rust.github.io/issues/";

/// Whether a project is a product or a demonstration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    /// Built for real use.
    Product,
    /// Built to show a technique.
    Demo,
}

/// Whether a project still receives changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaintainerStatus {
    /// Receives changes.
    Maintained,
    /// Complete and not expected to change.
    Finished,
}

/// The decision a judgment records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum JudgmentKind {
    /// A reviewed fact is wrong in the catalogue.
    Correctness {
        /// What the review found.
        finding: String,
    },
    /// Another entry replaces this one.
    Supersedes {
        /// The replacing entry.
        by: Slug,
    },
    /// The project kind changes.
    Kind {
        /// The new kind.
        value: ProjectKind,
    },
    /// The topics change.
    Category {
        /// The new topic ids.
        topics: Vec<Slug>,
    },
    /// The entry moves to a lower tier.
    Demotion {
        /// The current tier. Always `featured`.
        from: Tier,
        /// The new tier.
        to: Tier,
    },
    /// The maintenance status changes. The crate owner can report it.
    Maintainer {
        /// The new status.
        status: MaintainerStatus,
    },
    /// A maintainer outside the organization approves Featured for an
    /// `org_affiliated` entry. The variant carries no fields.
    FeaturedApproval {},
}

impl JudgmentKind {
    /// Whether this is a `maintainer` judgment.
    pub fn is_maintainer(&self) -> bool {
        matches!(self, Self::Maintainer { .. })
    }
}

/// A recorded decision about one entry.
///
/// The issue number comes from the file name `data/judgments/<number>.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Judgment {
    /// The entry this judgment applies to.
    pub entry: Slug,
    /// The issue that raised it.
    pub issue: Url,
    /// Who raised the issue.
    pub reporter: GithubLogin,
    /// The maintainer who approved it.
    pub approver: GithubLogin,
    /// Approval date.
    pub date: Date,
    /// Links that support the decision. A `maintainer` judgment can omit them.
    #[serde(default)]
    pub evidence: Vec<Url>,
    /// The decision.
    pub judgment: JudgmentKind,
}

impl Judgment {
    /// Whether `issue` is the repository issue with this number.
    pub fn issue_matches(&self, number: u64) -> bool {
        self.issue.as_str() == format!("{ISSUE_URL_PREFIX}{number}")
    }

    /// Lists every rule this judgment breaks on its own, without other files.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.evidence.is_empty() && !self.judgment.is_maintainer() {
            out.push("evidence needs at least one link".to_owned());
        }
        for link in &self.evidence {
            if link.scheme() != "https" {
                out.push(format!("evidence link `{link}` must use https"));
            }
        }
        match &self.judgment {
            JudgmentKind::Correctness { finding } if finding.trim().is_empty() => {
                out.push("finding must not be empty".to_owned());
            }
            JudgmentKind::Category { topics } if topics.is_empty() => {
                out.push("category needs at least one topic".to_owned());
            }
            JudgmentKind::Demotion { from, to } => {
                if *from != Tier::Featured {
                    out.push(format!(
                        "demotion must start from `featured`, not `{}`",
                        from.as_str()
                    ));
                }
                if from == to {
                    out.push("demotion must change the tier".to_owned());
                }
            }
            _ => {}
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = r#"entry = "alpha"
issue = "https://github.com/ml-rust/ml-rust.github.io/issues/7"
reporter = "dave"
approver = "carol"
date = 2026-11-03
"#;
    const EVIDENCE: &str = "evidence = [\"https://example.com/proof\"]\n";

    fn doc(evidence: bool, body: &str) -> String {
        let evidence = if evidence { EVIDENCE } else { "" };
        format!("{HEAD}{evidence}\n[judgment]\n{body}")
    }

    fn parse(text: &str) -> Result<Judgment, toml::de::Error> {
        toml::from_str(text)
    }

    fn problems(text: &str) -> Vec<String> {
        parse(text).expect("judgment parses").problems()
    }

    fn has(problems: &[String], needle: &str) -> bool {
        problems.iter().any(|p| p.contains(needle))
    }

    #[test]
    fn every_type_parses_and_passes() {
        for body in [
            "type = \"correctness\"\nfinding = \"The claim is wrong.\"\n",
            "type = \"supersedes\"\nby = \"beta\"\n",
            "type = \"kind\"\nvalue = \"demo\"\n",
            "type = \"category\"\ntopics = [\"tensors\"]\n",
            "type = \"demotion\"\nfrom = \"featured\"\nto = \"listed\"\n",
            "type = \"maintainer\"\nstatus = \"finished\"\n",
            "type = \"featured-approval\"\n",
        ] {
            assert!(problems(&doc(true, body)).is_empty(), "{body}");
        }
    }

    #[test]
    fn unknown_type_and_fields_are_rejected() {
        assert!(parse(&doc(true, "type = \"promote\"\n")).is_err());
        assert!(parse(&doc(true, "type = \"kind\"\nvalue = \"demo\"\nextra = 1\n")).is_err());
        assert!(
            parse(&format!(
                "extra = 1\n{}",
                doc(true, "type = \"kind\"\nvalue = \"demo\"\n")
            ))
            .is_err()
        );
    }

    #[test]
    fn featured_approval_has_no_fields() {
        let judgment = parse(&doc(true, "type = \"featured-approval\"\n")).expect("parses");
        assert_eq!(judgment.judgment, JudgmentKind::FeaturedApproval {});
        assert!(
            parse(&doc(
                true,
                "type = \"featured-approval\"\nto = \"featured\"\n"
            ))
            .is_err()
        );
        let text = toml::to_string(&judgment).expect("serializes");
        assert!(text.contains("type = \"featured-approval\""), "{text}");
    }

    #[test]
    fn featured_approval_needs_evidence() {
        let body = "type = \"featured-approval\"\n";
        assert!(has(&problems(&doc(false, body)), "evidence"));
    }

    #[test]
    fn evidence_is_required_except_for_maintainer() {
        let kind = "type = \"kind\"\nvalue = \"demo\"\n";
        assert!(has(&problems(&doc(false, kind)), "evidence"));
        let maintainer = "type = \"maintainer\"\nstatus = \"maintained\"\n";
        assert!(problems(&doc(false, maintainer)).is_empty());
    }

    #[test]
    fn evidence_must_use_https() {
        let text = doc(true, "type = \"kind\"\nvalue = \"demo\"\n")
            .replace("https://example.com/proof", "http://example.com/proof");
        let found = problems(&text);
        assert!(
            has(&found, "`http://example.com/proof` must use https"),
            "{found:?}"
        );
    }

    #[test]
    fn empty_finding_is_reported() {
        let body = "type = \"correctness\"\nfinding = \"  \"\n";
        assert!(has(&problems(&doc(true, body)), "finding"));
    }

    #[test]
    fn empty_category_is_reported() {
        let body = "type = \"category\"\ntopics = []\n";
        assert!(has(&problems(&doc(true, body)), "at least one topic"));
    }

    #[test]
    fn demotion_rules_are_checked() {
        let not_featured = "type = \"demotion\"\nfrom = \"listed\"\nto = \"archived\"\n";
        assert!(has(
            &problems(&doc(true, not_featured)),
            "start from `featured`"
        ));
        let same = "type = \"demotion\"\nfrom = \"featured\"\nto = \"featured\"\n";
        assert!(has(&problems(&doc(true, same)), "change the tier"));
    }

    #[test]
    fn issue_link_must_match_the_number() {
        let judgment = parse(&doc(true, "type = \"kind\"\nvalue = \"demo\"\n")).expect("parses");
        assert!(judgment.issue_matches(7));
        assert!(!judgment.issue_matches(8));
        assert!(!judgment.issue_matches(70));
    }
}
