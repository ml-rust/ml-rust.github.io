//! The report record of one entry.

use serde::{Deserialize, Serialize};

use super::fact::SignalFact;
use crate::tier::{Reason, Rule, Tier, Verdict};

/// One rule outcome with the values it compared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasonRecord {
    /// The rule.
    pub rule: Rule,
    /// The outcome and the values compared.
    pub detail: String,
}

impl From<&Reason> for ReasonRecord {
    fn from(reason: &Reason) -> Self {
        Self {
            rule: reason.rule,
            detail: reason.detail.clone(),
        }
    }
}

/// The tier of one entry, the reasons for it and every signal behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryReport {
    /// The published tier.
    pub tier: Tier,
    /// Whether the entry is finished: dormant by design, not decayed.
    pub stable: bool,
    /// Whether the dormancy rule finds the entry dormant. `None` when unknown.
    pub dormant: Option<bool>,
    /// The lower tier a Featured entry waits for while its demotion needs approval.
    pub demotion_pending: Option<Tier>,
    /// Rule outcomes in rule order.
    pub reasons: Vec<ReasonRecord>,
    /// Signals a rule needed but did not have, each named once, in first-use order.
    pub unknown: Vec<String>,
    /// One line per input the check left unresolved.
    pub notes: Vec<String>,
    /// Every signal with its raw value, source link and fetch date.
    pub signals: Vec<SignalFact>,
    /// The crate keywords, crate categories and repository topics, deduplicated, lowercase and
    /// sorted. Search input only: no tier rule reads them, and no page states them as a claim.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search_terms: Vec<String>,
}

impl EntryReport {
    /// The record of `verdict`, with the check's `notes` and the entry's `signals`.
    pub fn new(verdict: &Verdict, notes: Vec<String>, signals: Vec<SignalFact>) -> Self {
        Self {
            tier: verdict.tier,
            stable: verdict.stable,
            dormant: verdict.dormant,
            demotion_pending: verdict.demotion_pending,
            reasons: verdict.reasons.iter().map(ReasonRecord::from).collect(),
            unknown: verdict
                .unknown
                .iter()
                .map(|&name| name.to_owned())
                .collect(),
            notes,
            signals,
            search_terms: Vec::new(),
        }
    }

    /// The value of the signal `name`. `None` when no fact carries it, or the fact has no value.
    pub fn value_of(&self, name: &str) -> Option<&str> {
        self.signals
            .iter()
            .find(|fact| fact.name == name)
            .and_then(|fact| fact.value.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_the_verdict_and_keeps_the_notes_and_signals() {
        let verdict = Verdict {
            tier: Tier::Featured,
            stable: true,
            dormant: Some(true),
            dead_now: false,
            reasons: vec![Reason::new(Rule::DemotionPending, "held")],
            unknown: vec!["repo_gone"],
            demotion_pending: Some(Tier::Listed),
        };
        let report = EntryReport::new(&verdict, vec!["note".to_owned()], Vec::new());
        assert_eq!(report.tier, Tier::Featured);
        assert!(report.stable);
        assert_eq!(report.dormant, Some(true));
        assert_eq!(report.demotion_pending, Some(Tier::Listed));
        assert_eq!(
            report.reasons,
            [ReasonRecord {
                rule: Rule::DemotionPending,
                detail: "held".to_owned(),
            }]
        );
        assert_eq!(report.unknown, ["repo_gone"]);
        assert_eq!(report.notes, ["note"]);
        assert!(report.signals.is_empty());
        assert!(report.search_terms.is_empty());
    }
}
