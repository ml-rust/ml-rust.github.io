use super::level::Tier;

/// A tier rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rule {
    /// The author or RustSec ends the project: archived repo, `unmaintained`
    /// advisory, deprecation notice, every version yanked, or crate deleted.
    InstantArchive,
    /// The expected update interval E in days.
    ExpectedInterval,
    /// Days since the latest release or human commit, against E.
    Dormancy,
    /// One rot signal found.
    Rot,
    /// Dormant with enough rot signals. Archives after enough consecutive checks.
    Dead,
    /// Dormant without rot and still adopted, or a `finished` statement without rot.
    Finished,
    /// A young, prerelease or barely used project.
    New,
    /// The adoption and health gates for Featured.
    Featured,
    /// An org affiliated entry passes Featured but lacks an outside approval.
    FeaturedNeedsOutsideApproval,
    /// No other tier applies.
    Listed,
    /// A Featured entry keeps its tier until a demotion judgment approves the drop.
    DemotionPending,
}

impl Rule {
    /// The kebab-case rule name shown on entry pages.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InstantArchive => "instant-archive",
            Self::ExpectedInterval => "expected-interval",
            Self::Dormancy => "dormancy",
            Self::Rot => "rot",
            Self::Dead => "dead",
            Self::Finished => "finished",
            Self::New => "new",
            Self::Featured => "featured",
            Self::FeaturedNeedsOutsideApproval => "featured-needs-outside-approval",
            Self::Listed => "listed",
            Self::DemotionPending => "demotion-pending",
        }
    }
}

/// One rule outcome with the evidence values it used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    /// The rule.
    pub rule: Rule,
    /// The outcome and the values compared.
    pub detail: String,
}

impl Reason {
    /// A reason for `rule` with `detail`.
    pub fn new(rule: Rule, detail: impl Into<String>) -> Self {
        Self {
            rule,
            detail: detail.into(),
        }
    }
}

/// The tier of one entry on one check, with the reasons for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// The published tier.
    pub tier: Tier,
    /// Whether the entry is finished: dormant by design, not decayed.
    pub stable: bool,
    /// Whether this check finds the entry dead. Feeds the next check's `prior_dead_checks`.
    pub dead_now: bool,
    /// Rule outcomes in rule order.
    pub reasons: Vec<Reason>,
    /// Signals a rule needed but did not have, each named once, in first-use order.
    pub unknown: Vec<&'static str>,
    /// The lower tier the rules give a Featured entry while its demotion waits for approval.
    pub demotion_pending: Option<Tier>,
}

impl Verdict {
    /// Whether any reason comes from `rule`.
    pub fn has(&self, rule: Rule) -> bool {
        self.reasons.iter().any(|reason| reason.rule == rule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_names_are_kebab_case_and_distinct() {
        let rules = [
            Rule::InstantArchive,
            Rule::ExpectedInterval,
            Rule::Dormancy,
            Rule::Rot,
            Rule::Dead,
            Rule::Finished,
            Rule::New,
            Rule::Featured,
            Rule::FeaturedNeedsOutsideApproval,
            Rule::Listed,
            Rule::DemotionPending,
        ];
        let mut names: Vec<&str> = rules.iter().map(|rule| rule.as_str()).collect();
        assert!(
            names
                .iter()
                .all(|name| { name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') })
        );
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), rules.len());
    }

    #[test]
    fn has_finds_a_rule() {
        let verdict = Verdict {
            tier: Tier::Listed,
            stable: false,
            dead_now: false,
            reasons: vec![Reason::new(Rule::Listed, "listed")],
            unknown: Vec::new(),
            demotion_pending: None,
        };
        assert!(verdict.has(Rule::Listed));
        assert!(!verdict.has(Rule::New));
    }
}
