use super::level::Tier;
use crate::date::Date;
use crate::judgment::MaintainerStatus;

/// Facts about an entry that come from history and judgments, not from fetched signals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// The date of this check.
    pub today: Date,
    /// The tier the previous check published.
    pub previous_tier: Option<Tier>,
    /// Consecutive previous checks that found the entry dead.
    pub prior_dead_checks: u32,
    /// Whether this run fetched anything for the entry. A run that reuses stored values is no
    /// check, so it adds nothing to the dead count.
    pub checked: bool,
    /// The latest maintainer statement judgment.
    pub maintainer_statement: Option<MaintainerStatus>,
    /// Whether an approved demotion judgment exists.
    pub demotion_approved: bool,
    /// Whether an outside maintainer approved Featured.
    pub featured_approved: bool,
    /// Whether the ml-rust organization maintains the entry.
    pub org_affiliated: bool,
}

impl Context {
    /// A first check on `today`: no history, no judgments, not org affiliated.
    pub fn new(today: Date) -> Self {
        Self {
            today,
            previous_tier: None,
            prior_dead_checks: 0,
            checked: true,
            maintainer_statement: None,
            demotion_approved: false,
            featured_approved: false,
            org_affiliated: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_no_history_or_judgments() {
        let today = Date::new(2026, 11, 3).expect("date");
        let context = Context::new(today);
        assert_eq!(context.today, today);
        assert!(context.previous_tier.is_none());
        assert_eq!(context.prior_dead_checks, 0);
        assert!(context.maintainer_statement.is_none());
        assert!(!context.demotion_approved);
        assert!(!context.featured_approved);
        assert!(!context.org_affiliated);
    }
}
