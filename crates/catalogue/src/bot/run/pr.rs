//! Opens the data PR of an approved issue, inside the poll pass.
//!
//! GitHub is the source of truth: the issue events, the PRs on the bot
//! branch and a fresh read of the issue and `main` decide. The state file
//! plays no part.

use std::borrow::Cow;

use catalogue_bot::{
    APPROVED, Account, CrateOwner, CrateOwners, EventActor, ExistingPr, IssueEvent, IssueMeta,
    Note, OwnerFacts, Ownership, PR_OPEN, PrDecision, PrInput, PrIssue, PrPlan, PrStatus,
    PullFacts, RepoFacts, Sha256, approval_event, branch_name, decide, existing_pr, membership_org,
    precheck, prepare,
};
use catalogue_core::{CommitSha, DataSources, Entry, GithubLogin};
use catalogue_github::{ApiError, BranchWrite, Issue, NewPull, RepoClient};
use catalogue_signals::crates_io::CratesIo;

use super::error::IssueError;
use super::issue::RunContext;
use super::pass::MAIN_BRANCH;
use crate::tiers::github_repo;

/// What the PR step did for one issue.
#[derive(Debug, Default)]
pub(super) struct PrOutcome {
    /// The PR status the comment shows. `None` shows nothing.
    pub status: Option<PrStatus>,
    /// The one-time note when the bot removed `approved`.
    pub note: Option<Note>,
    /// What changed, for the run summary.
    pub changes: Vec<String>,
}

impl PrOutcome {
    /// An outcome that shows `status` and records `change`.
    fn showing(status: PrStatus, change: String) -> Self {
        Self {
            status: Some(status),
            note: None,
            changes: vec![change],
        }
    }
}

/// Opens the data PR of `issue` when every rule holds.
///
/// The caller calls it only when the issue carries `approved`, the report of
/// this pass does not block, and `checked_body` is the digest of the body the
/// report read. The step reads the issue events, the PRs on the bot branch,
/// the issue again and the head of `main`, then writes the branch, the PR and
/// the `pr-open` label. A refused approval loses its `approved` label.
pub(super) fn open_pr(
    client: &mut RepoClient<'_>,
    crates: &mut CratesIo<'_>,
    context: &RunContext<'_>,
    issue: &Issue,
    meta: &IssueMeta,
    checked_body: Sha256,
    blocking: bool,
) -> Result<PrOutcome, IssueError> {
    let number = meta.number.get();
    let api = |source| IssueError::Api { number, source };
    let events: Vec<IssueEvent> = client
        .issue_events(number)
        .map_err(api)?
        .into_iter()
        .map(issue_event)
        .collect();
    let approved = issue.labels.iter().any(|label| label == APPROVED);
    let approval = approval_event(&events, approved);
    let event = match precheck(issue.open, &issue.labels, blocking, &approval) {
        Ok((_, event)) => event,
        Err(decision) => return settle(client, number, &issue.labels, None, decision).map_err(api),
    };
    let branch = branch_name(meta.number);
    let pulls: Vec<PullFacts> = client
        .pulls_for_branch(&branch)
        .map_err(api)?
        .into_iter()
        .filter(|pull| pull.head_ref == branch)
        .map(|pull| PullFacts {
            number: pull.number,
            open: pull.open,
            merged: pull.merged,
            closed_at: pull.closed_at,
        })
        .collect();
    match existing_pr(&pulls, event) {
        ExistingPr::Open(found) => {
            let mut outcome =
                PrOutcome::showing(PrStatus::Open(found), format!("PR #{found} open"));
            if !issue.labels.iter().any(|label| label == PR_OPEN) {
                client
                    .add_labels(number, &[PR_OPEN.to_owned()])
                    .map_err(api)?;
                outcome.changes.push(format!("added {PR_OPEN}"));
            }
            return Ok(outcome);
        }
        ExistingPr::Merged(found) => {
            return Ok(PrOutcome::showing(
                PrStatus::Merged(found),
                format!("PR #{found} merged"),
            ));
        }
        ExistingPr::Closed(found) => {
            return Ok(PrOutcome::showing(
                PrStatus::Closed(found),
                format!("PR #{found} closed, approval needs a new label add"),
            ));
        }
        ExistingPr::None => {}
    }
    let fresh = client.issue(number).map_err(api)?;
    let (main, main_sha) = current_main(client, context).map_err(api)?;
    let input = PrInput {
        meta,
        issue: PrIssue {
            open: fresh.open,
            labels: &fresh.labels,
            body: &fresh.body,
        },
        blocking,
        checked_body,
        approval: &approval,
        main: &main,
        today: context.today,
    };
    let prepared = match prepare(&input) {
        Ok(prepared) => prepared,
        Err(decision) => {
            return settle(client, number, &fresh.labels, None, decision).map_err(api);
        }
    };
    let ownership = match prepared.owner_entry() {
        Some(entry) => Some(ownership(client, crates, entry, &meta.author, number)?),
        None => None,
    };
    let decision = prepared.finish(ownership.as_ref());
    settle(client, number, &fresh.labels, Some(&main_sha), decision).map_err(api)
}

/// The issue event in the form the approval rules read.
fn issue_event(found: catalogue_github::IssueEvent) -> IssueEvent {
    IssueEvent {
        id: found.id,
        event: found.event,
        actor: found.actor.map(|actor| EventActor {
            login: actor.login,
            kind: actor.kind,
        }),
        label: found.label,
        created_at: found.created_at,
    }
}

/// The data files at the head of `main` and their commit.
///
/// The files of the pass serve when `main` has not moved. Otherwise the step reads them again.
fn current_main<'a>(
    client: &mut RepoClient<'_>,
    context: &RunContext<'a>,
) -> Result<(Cow<'a, DataSources>, CommitSha), ApiError> {
    let sha = client.branch_sha(MAIN_BRANCH)?;
    if &sha == context.main_sha {
        return Ok((Cow::Borrowed(context.main), sha));
    }
    let data = client.branch_data(MAIN_BRANCH)?;
    Ok((Cow::Owned(data.sources), data.sha))
}

/// Reads the ownership facts of `entry` and applies the rules for `author`.
///
/// Public membership is read only when the first two rules fail and the
/// third rule names an org.
fn ownership(
    client: &mut RepoClient<'_>,
    crates: &mut CratesIo<'_>,
    entry: &Entry,
    author: &GithubLogin,
    number: u64,
) -> Result<Ownership, IssueError> {
    let api = |source| IssueError::Api { number, source };
    let mut owners = Vec::with_capacity(entry.crates.len());
    for name in &entry.crates {
        let found = crates
            .owners(name.as_str())
            .map_err(|source| IssueError::CratesIo { number, source })?;
        owners.push(CrateOwners {
            name: name.clone(),
            owners: found
                .into_iter()
                .map(|owner| CrateOwner {
                    kind: owner.kind,
                    login: owner.login,
                })
                .collect(),
        });
    }
    let repo = match github_repo(entry) {
        Ok(repo) => Some(RepoFacts {
            full_name: repo.to_string(),
            owner: client.repo_owner(&repo).map_err(api)?.map(|owner| Account {
                login: owner.login,
                kind: owner.kind,
            }),
        }),
        Err(_) => None,
    };
    let mut facts = OwnerFacts {
        crates: owners,
        repo,
        public_member: None,
    };
    let decided = decide(author, &facts);
    if matches!(decided, Ownership::Confirmed(_)) {
        return Ok(decided);
    }
    let Some(team) = membership_org(&facts) else {
        return Ok(decided);
    };
    let member = client
        .public_member(team.org.as_str(), author.as_str())
        .map_err(api)?;
    facts.public_member = Some(member);
    Ok(decide(author, &facts))
}

/// Applies `decision` to issue `number`, which carries `labels`.
///
/// A plan opens the PR on `main_sha`. Only [`prepare`] yields a plan, and it
/// runs after `main` is read, so a plan without `main_sha` writes nothing.
fn settle(
    client: &mut RepoClient<'_>,
    number: u64,
    labels: &[String],
    main_sha: Option<&CommitSha>,
    decision: PrDecision,
) -> Result<PrOutcome, ApiError> {
    match (decision, main_sha) {
        (PrDecision::Open(plan), Some(sha)) => create(client, number, labels, sha, &plan),
        (PrDecision::Open(_) | PrDecision::Skip(_), _) => Ok(PrOutcome::default()),
        (PrDecision::Blocked(lines), _) => Ok(PrOutcome::showing(
            PrStatus::Blocked(lines),
            "PR blocked".to_owned(),
        )),
        (PrDecision::RemoveApproval(lines), _) => {
            client.remove_label(number, APPROVED)?;
            Ok(PrOutcome {
                status: None,
                note: Some(Note::Refused(lines)),
                changes: vec![format!("removed {APPROVED}, approval refused")],
            })
        }
    }
}

/// Builds one commit on `main_sha` that adds the file of `plan`, points the
/// bot branch at it, opens the PR and labels the issue `pr-open`.
fn create(
    client: &mut RepoClient<'_>,
    number: u64,
    labels: &[String],
    main_sha: &CommitSha,
    plan: &PrPlan,
) -> Result<PrOutcome, ApiError> {
    let base = client.commit_tree(main_sha)?;
    let tree = client.create_tree(&base, &plan.path, &plan.contents)?;
    let commit = client.create_commit(&plan.commit_message, &tree, main_sha)?;
    let branch = match client.point_branch(&plan.branch, &commit)? {
        BranchWrite::Created => "branch created",
        BranchWrite::Moved => "branch moved",
    };
    let pull = client.create_pull(&NewPull {
        title: &plan.title,
        body: &plan.body,
        head: &plan.branch,
        base: MAIN_BRANCH,
    })?;
    let mut changes = vec![branch.to_owned(), format!("opened PR #{pull}")];
    if !labels.iter().any(|label| label == PR_OPEN) {
        client.add_labels(number, &[PR_OPEN.to_owned()])?;
        changes.push(format!("added {PR_OPEN}"));
    }
    Ok(PrOutcome {
        status: Some(PrStatus::Open(pull)),
        note: None,
        changes,
    })
}
