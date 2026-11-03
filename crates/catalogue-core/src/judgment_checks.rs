use crate::catalogue::Catalogue;
use crate::ids::{GithubLogin, Slug};
use crate::judgment::{Judgment, JudgmentKind};
use crate::load;

impl Catalogue {
    /// Lists the reference rules that a candidate `kind` judgment on `entry` breaks.
    ///
    /// Checks that the entry, the replacing entry and the topics exist. Also
    /// reports a replacement cycle that the judgment closes. The rules
    /// need no approver, so a judgment still waiting for approval can be checked.
    pub fn judgment_reference_problems(&self, entry: &Slug, kind: &JudgmentKind) -> Vec<String> {
        load::reference_problems(self, entry, kind)
    }

    /// Lists the approval rules that a candidate `judgment` breaks.
    ///
    /// Checks the approver against the maintainer list. Also checks the
    /// reporter rule and the org conflict rule.
    pub fn approval_problems(&self, judgment: &Judgment) -> Vec<String> {
        load::approval_problems(self, judgment)
    }

    /// The maintainers who can approve `kind` on `entry` when `reporter` raised it.
    ///
    /// The list is empty when no approver can satisfy the approval rules.
    pub fn eligible_approvers(
        &self,
        entry: &Slug,
        reporter: &GithubLogin,
        kind: &JudgmentKind,
    ) -> Vec<GithubLogin> {
        load::eligible_approvers(self, entry, reporter, kind)
    }

    /// The maintainers who can approve a new entry that `reporter` submitted.
    ///
    /// The reporter cannot approve. An org member cannot approve an entry with
    /// `org_affiliated` set. `entry` names the entry in messages only.
    pub fn submission_approvers(
        &self,
        entry: &str,
        org_affiliated: bool,
        reporter: &GithubLogin,
    ) -> Vec<GithubLogin> {
        load::submission_approvers(self, entry, org_affiliated, reporter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{JUDGMENT, put, supersedes_judgment, valid};

    fn judgment(text: &str) -> Judgment {
        toml::from_str(text).expect("judgment parses")
    }

    fn slug(text: &str) -> Slug {
        Slug::new(text).expect("slug")
    }

    fn login(text: &str) -> GithubLogin {
        GithubLogin::new(text).expect("login")
    }

    fn finding() -> JudgmentKind {
        JudgmentKind::Correctness {
            finding: "Wrong.".to_owned(),
        }
    }

    fn catalogue() -> Catalogue {
        Catalogue::load(valid()).expect("valid data")
    }

    fn logins(found: Vec<GithubLogin>) -> Vec<String> {
        found.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_sound_candidate_has_no_problems() {
        let candidate = judgment(JUDGMENT);
        assert!(
            catalogue()
                .judgment_reference_problems(&candidate.entry, &candidate.judgment)
                .is_empty()
        );
        assert!(catalogue().approval_problems(&candidate).is_empty());
    }

    #[test]
    fn unknown_entry_is_a_reference_problem() {
        let candidate = judgment(&JUDGMENT.replace("entry = \"alpha\"", "entry = \"ghost\""));
        let found = catalogue().judgment_reference_problems(&candidate.entry, &candidate.judgment);
        assert_eq!(found, ["entry `ghost` does not exist"]);
    }

    #[test]
    fn a_closing_replacement_is_a_cycle() {
        let mut src = valid();
        put(
            &mut src,
            "judgments/8.toml",
            &supersedes_judgment(8, "alpha", "beta"),
        );
        let catalogue = Catalogue::load(src).expect("valid data");
        let candidate = judgment(&supersedes_judgment(9, "beta", "alpha"));
        let found = catalogue.judgment_reference_problems(&candidate.entry, &candidate.judgment);
        assert!(
            found.iter().any(|m| m.contains("forms a cycle")),
            "{found:?}"
        );
    }

    #[test]
    fn unlisted_approver_is_an_approval_problem() {
        let candidate = judgment(&JUDGMENT.replace("approver = \"carol\"", "approver = \"zed\""));
        let found = catalogue().approval_problems(&candidate);
        assert_eq!(found, ["approver `zed` is not a maintainer"]);
    }

    #[test]
    fn eligible_approvers_follow_the_approval_rules() {
        let catalogue = catalogue();
        let eligible = |entry: &str, reporter: &str| {
            logins(catalogue.eligible_approvers(&slug(entry), &login(reporter), &finding()))
        };
        assert_eq!(eligible("alpha", "dave"), ["carol", "bob"]);
        assert_eq!(eligible("alpha", "carol"), ["bob"]);
        assert_eq!(eligible("beta", "dave"), ["bob"]);
    }

    #[test]
    fn featured_approval_on_an_unaffiliated_entry_has_no_approver() {
        let found = catalogue().eligible_approvers(
            &slug("alpha"),
            &login("dave"),
            &JudgmentKind::FeaturedApproval {},
        );
        assert!(found.is_empty());
    }

    #[test]
    fn submission_approvers_follow_the_reporter_and_conflict_rules() {
        let catalogue = catalogue();
        let approvers = |affiliated: bool, reporter: &str| {
            logins(catalogue.submission_approvers("new", affiliated, &login(reporter)))
        };
        assert_eq!(approvers(false, "dave"), ["carol", "bob"]);
        assert_eq!(approvers(false, "Carol"), ["bob"]);
        assert_eq!(approvers(true, "dave"), ["bob"]);
        assert!(approvers(true, "bob").is_empty());
    }
}
