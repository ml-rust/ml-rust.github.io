//! Checks one issue and writes its comment and labels.

use std::num::NonZeroU64;

use catalogue_bot::{
    APPROVED, CheckInput, CheckReport, CommentAction, Desired, Form, FormError, FormKind,
    IssueMeta, Marker, Note, Previous, Sha256, approval_stale, check_submission, comment_body,
    label_report, parse_form, plan, posted_section, report_digest,
};
use catalogue_core::{CommitSha, DataSources, Date, GithubLogin, Tier};
use catalogue_github::{ApiError, Comment, CommentUpdate, Issue, RepoClient};
use catalogue_signals::crates_io::CratesIo;
use catalogue_signals::history::Snapshot;

use super::error::IssueError;
use super::pr::{PrOutcome, open_pr};
use super::state::IssueState;
use super::triage::{LinkCheck, PreviousComment, triage_issue};

/// What every issue check of one run shares.
pub(super) struct RunContext<'a> {
    /// The App slug. The bot account is `<slug>[bot]`.
    pub slug: &'a str,
    /// The data files on `main`.
    pub main: &'a DataSources,
    /// The commit of `main` the data files come from.
    pub main_sha: &'a CommitSha,
    /// The newest tier snapshot. `None` when there is none.
    pub snapshot: Option<&'a Snapshot>,
    /// The date that becomes `added` on a new entry.
    pub today: Date,
    /// The unix second of the check, for the link check wait.
    pub now: i64,
}

/// The result of one issue check.
pub(super) struct Checked {
    /// What the bot wrote, for the state file.
    pub state: IssueState,
    /// What changed on the issue, for the run summary.
    pub summary: String,
    /// Problems that did not stop the check, such as a failed link check.
    pub warnings: Vec<String>,
}

/// The bot comment already on an issue: its marker and its text.
struct Found {
    /// The comment id and marker.
    previous: Previous,
    /// The comment text, where the last link check section sits.
    text: String,
}

/// Whether `issue` carries a form label.
pub(super) fn has_form_label(issue: &Issue) -> bool {
    issue
        .labels
        .iter()
        .any(|label| FormKind::from_label(label).is_some())
}

/// Checks `issue`, opens its data PR when it is approved, runs the link
/// check, and applies the writes the plan asks for.
///
/// `cached` is the state from the last run. It names the comment to read
/// first. The marker in the comment decides, never the cache. The PR step
/// runs only when the issue carries `approved`, the report does not block,
/// and the approval still covers the checked body. Its status goes into the
/// comment text and the marker digest. The link check runs only with a
/// `link_check`. Its section goes last in the comment and into the marker digest.
pub(super) fn check_issue(
    client: &mut RepoClient<'_>,
    crates: &mut CratesIo<'_>,
    context: &RunContext<'_>,
    issue: &Issue,
    cached: Option<&IssueState>,
    link_check: Option<&mut LinkCheck<'_>>,
) -> Result<Checked, IssueError> {
    let number = issue.number;
    let api = |source| IssueError::Api { number, source };
    let meta = issue_meta(issue)?;
    let (report, form) = build_report(context, issue, &meta);
    let body = Sha256::of(issue.body.as_bytes());
    let found = find_previous(client, context.slug, number, cached).map_err(api)?;
    let previous = found.as_ref().map(|found| &found.previous);
    let stale = approval_stale(previous, &body, &issue.labels);
    let approved = issue.labels.iter().any(|label| label == APPROVED);
    let pr = if approved && !stale && !report.blocking {
        open_pr(client, crates, context, issue, &meta, body, report.blocking)?
    } else {
        PrOutcome::default()
    };
    let mut notes = Vec::new();
    if stale {
        notes.push(Note::BodyChanged);
    }
    let refused = pr.note.is_some();
    notes.extend(pr.note);
    let triaged = triage_issue(
        link_check,
        client,
        form.as_ref(),
        found.as_ref().map(|found| PreviousComment {
            marker: &found.previous.marker,
            text: &found.text,
        }),
        cached,
        context.now,
        number,
    );
    let section = posted_section(
        &report,
        pr.status.as_ref(),
        &notes,
        triaged.section.as_deref(),
    );
    let section = section.as_deref();
    let desired = Desired {
        marker: Marker {
            body,
            report: report_digest(&report, pr.status.as_ref(), section),
            main: Some(context.main_sha.clone()),
            triage: triaged.digest,
        },
        blocking: report.blocking,
        note: refused,
    };
    let actions = plan(&desired, previous, &issue.labels);
    let mut changes = pr.changes;
    if triaged.ran {
        changes.push("links checked".to_owned());
    }
    if actions.drop_approved {
        client.remove_label(number, APPROVED).map_err(api)?;
        changes.push(format!("removed {APPROVED}"));
    }
    let text = comment_body(
        &desired.marker,
        &report,
        pr.status.as_ref(),
        &notes,
        section,
    );
    let comment_id = match (actions.comment, &previous) {
        (CommentAction::Skip, Some(found)) => found.comment_id,
        (CommentAction::Update(id), _) => {
            changes.push("comment updated".to_owned());
            update_or_post(client, context.slug, number, id, &text).map_err(api)?
        }
        (CommentAction::Skip | CommentAction::Create, _) => {
            changes.push("comment posted".to_owned());
            client.create_comment(number, &text).map_err(api)?
        }
    };
    for name in &actions.remove_labels {
        client.remove_label(number, name).map_err(api)?;
        changes.push(format!("removed {name}"));
    }
    if !actions.add_labels.is_empty() {
        client
            .add_labels(number, &actions.add_labels)
            .map_err(api)?;
        changes.extend(
            actions
                .add_labels
                .iter()
                .map(|name| format!("added {name}")),
        );
    }
    let summary = if changes.is_empty() {
        format!("issue {number}: up to date")
    } else {
        format!("issue {number}: {}", changes.join(", "))
    };
    Ok(Checked {
        state: IssueState {
            comment_id,
            body_sha256: desired.marker.body.to_string(),
            report_sha256: desired.marker.report.to_string(),
            main: desired.marker.main.as_ref().map(ToString::to_string),
            triage_sha256: desired.marker.triage.as_ref().map(ToString::to_string),
            triage_retry_after: triaged.retry_after,
        },
        summary,
        warnings: triaged.warning.into_iter().collect(),
    })
}

/// The number and author of `issue`, from the API fields.
fn issue_meta(issue: &Issue) -> Result<IssueMeta, IssueError> {
    let number = issue.number;
    let author = issue
        .author
        .as_ref()
        .and_then(|author| GithubLogin::new(author.login.as_str()).ok());
    match (author, NonZeroU64::new(number)) {
        (Some(author), Some(number)) => Ok(IssueMeta { number, author }),
        _ => Err(IssueError::Author { number }),
    }
}

/// The check report of `issue` and its parsed form: the form check, or the
/// label error when its labels name no form or several.
///
/// The form is `None` when the labels name no single form or the body does not parse.
fn build_report(
    context: &RunContext<'_>,
    issue: &Issue,
    meta: &IssueMeta,
) -> (CheckReport, Option<Form>) {
    let kind = match FormKind::from_labels(issue.labels.iter().map(String::as_str)) {
        Ok(kind) => kind,
        Err(error) => return (label_report(&error), None),
    };
    let form = parse_form(kind, &issue.body);
    let report = check_submission(&CheckInput {
        main: context.main,
        kind,
        form: &form,
        meta,
        today: context.today,
        current_tier: entry_tier(&form, context.snapshot),
    });
    (report, form.ok())
}

/// The snapshot tier of the entry a judgment form names.
fn entry_tier(form: &Result<Form, Vec<FormError>>, snapshot: Option<&Snapshot>) -> Option<Tier> {
    let Ok(Form::Judgment(judgment)) = form else {
        return None;
    };
    snapshot?.rows.get(&judgment.entry).map(|row| row.tier)
}

/// The marker and text of `comment` when the App bot wrote it on issue `number`.
///
/// A comment by any other account, of any other type, or without a valid
/// marker on its first line gives `None`. A forged marker never counts.
fn bot_marker(comment: &Comment, slug: &str, number: u64) -> Option<Found> {
    let author = comment.author.as_ref()?;
    if !author.is_app_bot(slug) || !comment.is_on_issue(number) {
        return None;
    }
    Marker::of_comment(&comment.body).map(|marker| Found {
        previous: Previous {
            comment_id: comment.id,
            marker,
        },
        text: comment.body.clone(),
    })
}

/// The bot comment on issue `number`: the cached one when it still holds a
/// bot marker, else the first bot comment in the listing.
fn find_previous(
    client: &mut RepoClient<'_>,
    slug: &str,
    number: u64,
    cached: Option<&IssueState>,
) -> Result<Option<Found>, ApiError> {
    if let Some(cached) = cached
        && let Some(comment) = client.comment(cached.comment_id)?
        && let Some(found) = bot_marker(&comment, slug, number)
    {
        return Ok(Some(found));
    }
    first_bot_comment(client, slug, number, None)
}

/// The first comment on issue `number` with a bot marker, skipping comment `skip`.
fn first_bot_comment(
    client: &mut RepoClient<'_>,
    slug: &str,
    number: u64,
    skip: Option<u64>,
) -> Result<Option<Found>, ApiError> {
    Ok(client
        .issue_comments(number)?
        .iter()
        .filter(|comment| Some(comment.id) != skip)
        .find_map(|comment| bot_marker(comment, slug, number)))
}

/// Writes `text` over comment `id`. When the comment is gone, writes over the
/// next bot comment on the issue, else posts a new one. Returns the comment id.
fn update_or_post(
    client: &mut RepoClient<'_>,
    slug: &str,
    number: u64,
    id: u64,
    text: &str,
) -> Result<u64, ApiError> {
    if client.update_comment(id, text)? == CommentUpdate::Updated {
        return Ok(id);
    }
    if let Some(other) = first_bot_comment(client, slug, number, Some(id))?
        && client.update_comment(other.previous.comment_id, text)? == CommentUpdate::Updated
    {
        return Ok(other.previous.comment_id);
    }
    client.create_comment(number, text)
}
