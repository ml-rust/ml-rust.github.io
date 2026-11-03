//! Whether the author of a `judgment:maintainer` issue owns the entry.

use catalogue_core::{CrateName, GithubLogin};

/// The crates.io owner kind of a person.
const USER_KIND: &str = "user";
/// The crates.io owner kind of a GitHub team.
const TEAM_KIND: &str = "team";
/// The prefix of a crates.io team login: `github:<org>:<team>`.
const TEAM_PREFIX: &str = "github:";
/// The GitHub account type of a person.
const GITHUB_USER: &str = "User";
/// The GitHub account type of an organization.
const GITHUB_ORG: &str = "Organization";

/// The fix an unconfirmed author can make.
const FIX: &str = "To fix it, ask a crates.io owner of the crate to add you as an owner. \
Or make your membership of the org public. Then remove the `approved` label and add it again.";

/// One owner of a crate, as the crates.io API lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateOwner {
    /// The owner kind: `user` or `team`.
    pub kind: String,
    /// The login. A team login reads `github:<org>:<team>`.
    pub login: String,
}

/// The owners of one crate of the entry. Empty when crates.io does not know the crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateOwners {
    /// The crate.
    pub name: CrateName,
    /// Its owners.
    pub owners: Vec<CrateOwner>,
}

/// A GitHub account, as the API reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// The login.
    pub login: String,
    /// The account type: `User` or `Organization`.
    pub kind: String,
}

/// The GitHub repo of the entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFacts {
    /// `owner/name` from the entry link.
    pub full_name: String,
    /// The repo owner from `GET /repos/{owner}/{name}`. `None` when the repo is not found.
    pub owner: Option<Account>,
}

/// Everything the ownership rules read about the entry on `main`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerFacts {
    /// The owners of each crate of the entry, in entry order.
    pub crates: Vec<CrateOwners>,
    /// The repo, or `None` when the entry repo is not on GitHub.
    pub repo: Option<RepoFacts>,
    /// Whether the author is a public member of [`membership_org`]. `None` when not asked.
    pub public_member: Option<bool>,
}

/// The org membership the third rule needs: the org and the team that owns a crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamOwner {
    /// The org that owns the entry repo.
    pub org: GithubLogin,
    /// The crate the team owns.
    pub crate_name: CrateName,
    /// The team name inside the org.
    pub team: String,
}

/// Why the author counts as an owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Basis {
    /// The author is a user owner of this crate on crates.io.
    CrateOwner(CrateName),
    /// The author owns the entry repo, named `owner/name`.
    RepoOwner(String),
    /// The author is a public member of the org whose team owns a crate.
    OrgMember(TeamOwner),
}

impl Basis {
    /// One sentence that names the basis for `author`.
    pub fn sentence(&self, author: &GithubLogin) -> String {
        match self {
            Self::CrateOwner(name) => format!("`{author}` owns crate `{name}` on crates.io."),
            Self::RepoOwner(repo) => format!("`{author}` owns the repo `{repo}` on GitHub."),
            Self::OrgMember(team) => format!(
                "`{author}` is a public member of `{}`, and its team `{}` owns crate `{}` on crates.io.",
                team.org, team.team, team.crate_name
            ),
        }
    }
}

/// The result of the ownership rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ownership {
    /// The author owns the entry on this basis.
    Confirmed(Basis),
    /// No rule holds. The lines say what was checked and how to fix it.
    Unconfirmed(Vec<String>),
}

/// The org and team whose public membership the third rule needs, or `None`.
///
/// The entry repo must belong to an organization, and a crate of the entry
/// must have a team owner `github:<org>:<team>` from that same org.
pub fn membership_org(facts: &OwnerFacts) -> Option<TeamOwner> {
    let owner = facts.repo.as_ref()?.owner.as_ref()?;
    if owner.kind != GITHUB_ORG {
        return None;
    }
    let org = GithubLogin::new(owner.login.as_str()).ok()?;
    facts.crates.iter().find_map(|found| {
        found.owners.iter().find_map(|candidate| {
            let team = team_of(candidate, &org)?;
            Some(TeamOwner {
                org: org.clone(),
                crate_name: found.name.clone(),
                team,
            })
        })
    })
}

/// The team name of `owner` when it is a team of `org`.
fn team_of(owner: &CrateOwner, org: &GithubLogin) -> Option<String> {
    if owner.kind != TEAM_KIND {
        return None;
    }
    let (found_org, team) = owner.login.strip_prefix(TEAM_PREFIX)?.split_once(':')?;
    let safe = !team.is_empty()
        && team
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'));
    (found_org.eq_ignore_ascii_case(org.as_str()) && safe).then(|| team.to_owned())
}

/// Decides whether `author` owns the entry that `facts` describe.
///
/// Logins compare without regard to case. The rules, in order: a crates.io
/// user owner of any crate, the user that owns the repo, or a public member
/// of the repo org whose team owns a crate.
pub fn decide(author: &GithubLogin, facts: &OwnerFacts) -> Ownership {
    let same = |login: &str| login.eq_ignore_ascii_case(author.as_str());
    for found in &facts.crates {
        let owns = found
            .owners
            .iter()
            .any(|owner| owner.kind == USER_KIND && same(&owner.login));
        if owns {
            return Ownership::Confirmed(Basis::CrateOwner(found.name.clone()));
        }
    }
    if let Some(repo) = &facts.repo
        && let Some(owner) = &repo.owner
        && owner.kind == GITHUB_USER
        && same(&owner.login)
    {
        return Ownership::Confirmed(Basis::RepoOwner(repo.full_name.clone()));
    }
    let team = membership_org(facts);
    if let Some(team) = &team
        && facts.public_member == Some(true)
    {
        return Ownership::Confirmed(Basis::OrgMember(team.clone()));
    }
    Ownership::Unconfirmed(checked(author, facts, team.as_ref()))
}

/// What the rules checked, one line per rule, then the fix.
fn checked(author: &GithubLogin, facts: &OwnerFacts, team: Option<&TeamOwner>) -> Vec<String> {
    let names: Vec<String> = facts
        .crates
        .iter()
        .map(|found| format!("`{}`", found.name))
        .collect();
    let crates = if names.is_empty() {
        "crates.io: the entry lists no crate.".to_owned()
    } else {
        format!(
            "crates.io: `{author}` is not a user owner of {}.",
            names.join(", ")
        )
    };
    let repo = match &facts.repo {
        None => "GitHub: the entry repo is not on GitHub.".to_owned(),
        Some(RepoFacts {
            full_name,
            owner: None,
        }) => format!("GitHub: the repo `{full_name}` is not found."),
        Some(RepoFacts { full_name, .. }) => {
            format!("GitHub: `{author}` does not own the repo `{full_name}`.")
        }
    };
    let membership = match team {
        None => "Org membership: no crate of the entry has a crates.io team owner from the org that owns the repo.".to_owned(),
        Some(team) => format!(
            "Org membership: `{author}` is not a public member of `{}`.",
            team.org
        ),
    };
    vec![crates, repo, membership, FIX.to_owned()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login(text: &str) -> GithubLogin {
        GithubLogin::new(text).expect("login")
    }

    fn owner(kind: &str, login: &str) -> CrateOwner {
        CrateOwner {
            kind: kind.to_owned(),
            login: login.to_owned(),
        }
    }

    fn crate_owners(name: &str, owners: Vec<CrateOwner>) -> CrateOwners {
        CrateOwners {
            name: CrateName::new(name).expect("crate"),
            owners,
        }
    }

    fn repo(login: &str, kind: &str) -> Option<RepoFacts> {
        Some(RepoFacts {
            full_name: "acme/gizmo".to_owned(),
            owner: Some(Account {
                login: login.to_owned(),
                kind: kind.to_owned(),
            }),
        })
    }

    fn facts(crates: Vec<CrateOwners>, repo: Option<RepoFacts>) -> OwnerFacts {
        OwnerFacts {
            crates,
            repo,
            public_member: None,
        }
    }

    #[test]
    fn a_crate_user_owner_is_confirmed_without_regard_to_case() {
        let found = facts(
            vec![
                crate_owners("gizmo-core", vec![owner("user", "someone")]),
                crate_owners("gizmo-nn", vec![owner("user", "Dave")]),
            ],
            repo("acme", GITHUB_ORG),
        );
        assert_eq!(
            decide(&login("dave"), &found),
            Ownership::Confirmed(Basis::CrateOwner(
                CrateName::new("gizmo-nn").expect("crate")
            ))
        );
    }

    #[test]
    fn the_user_that_owns_the_repo_is_confirmed() {
        let found = facts(Vec::new(), repo("DAVE", GITHUB_USER));
        let decided = decide(&login("dave"), &found);
        assert_eq!(
            decided,
            Ownership::Confirmed(Basis::RepoOwner("acme/gizmo".to_owned()))
        );
        let Ownership::Confirmed(basis) = decided else {
            panic!("confirmed");
        };
        assert_eq!(
            basis.sentence(&login("dave")),
            "`dave` owns the repo `acme/gizmo` on GitHub."
        );
        let org_named_dave = facts(Vec::new(), repo("dave", GITHUB_ORG));
        assert!(matches!(
            decide(&login("dave"), &org_named_dave),
            Ownership::Unconfirmed(_)
        ));
    }

    #[test]
    fn a_public_member_of_the_team_org_is_confirmed() {
        let mut found = facts(
            vec![crate_owners(
                "gizmo-core",
                vec![owner("team", "github:ACME:core")],
            )],
            repo("acme", GITHUB_ORG),
        );
        let team = membership_org(&found).expect("membership needed");
        assert_eq!(team.org.as_str(), "acme");
        assert_eq!(team.team, "core");
        found.public_member = Some(true);
        assert_eq!(
            decide(&login("dave"), &found),
            Ownership::Confirmed(Basis::OrgMember(team.clone()))
        );
        assert_eq!(
            Basis::OrgMember(team).sentence(&login("dave")),
            "`dave` is a public member of `acme`, and its team `core` owns crate `gizmo-core` on crates.io."
        );
    }

    #[test]
    fn a_team_only_crate_without_public_membership_is_unconfirmed() {
        let mut found = facts(
            vec![crate_owners(
                "gizmo-core",
                vec![owner("team", "github:acme:core")],
            )],
            repo("acme", GITHUB_ORG),
        );
        found.public_member = Some(false);
        let Ownership::Unconfirmed(lines) = decide(&login("dave"), &found) else {
            panic!("unconfirmed");
        };
        assert_eq!(
            lines[0],
            "crates.io: `dave` is not a user owner of `gizmo-core`."
        );
        assert_eq!(
            lines[1],
            "GitHub: `dave` does not own the repo `acme/gizmo`."
        );
        assert_eq!(
            lines[2],
            "Org membership: `dave` is not a public member of `acme`."
        );
        assert!(lines[3].contains("remove the `approved` label and add it again"));
        let user_named_team = facts(
            vec![crate_owners(
                "gizmo-core",
                vec![owner("user", "github:acme:core")],
            )],
            repo("acme", GITHUB_ORG),
        );
        assert_eq!(membership_org(&user_named_team), None);
    }

    #[test]
    fn a_team_of_another_org_or_a_user_repo_needs_no_membership() {
        let other_org = facts(
            vec![crate_owners(
                "gizmo-core",
                vec![owner("team", "github:other:core")],
            )],
            repo("acme", GITHUB_ORG),
        );
        assert_eq!(membership_org(&other_org), None);
        let user_repo = facts(
            vec![crate_owners(
                "gizmo-core",
                vec![owner("team", "github:acme:core")],
            )],
            repo("acme", GITHUB_USER),
        );
        assert_eq!(membership_org(&user_repo), None);
        let odd_team = facts(
            vec![crate_owners(
                "gizmo-core",
                vec![owner("team", "github:acme:a`b")],
            )],
            repo("acme", GITHUB_ORG),
        );
        assert_eq!(membership_org(&odd_team), None);
    }

    #[test]
    fn missing_crates_and_a_missing_repo_are_unconfirmed() {
        let found = facts(
            vec![crate_owners("gizmo-core", Vec::new())],
            Some(RepoFacts {
                full_name: "acme/gizmo".to_owned(),
                owner: None,
            }),
        );
        let Ownership::Unconfirmed(lines) = decide(&login("dave"), &found) else {
            panic!("unconfirmed");
        };
        assert_eq!(lines[1], "GitHub: the repo `acme/gizmo` is not found.");
        assert!(
            lines[2].starts_with("Org membership: no crate"),
            "{lines:?}"
        );
        let Ownership::Unconfirmed(none) = decide(&login("dave"), &facts(Vec::new(), None)) else {
            panic!("unconfirmed");
        };
        assert_eq!(none[0], "crates.io: the entry lists no crate.");
        assert_eq!(none[1], "GitHub: the entry repo is not on GitHub.");
    }
}
