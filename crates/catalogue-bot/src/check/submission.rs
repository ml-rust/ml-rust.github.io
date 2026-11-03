//! Builds the check report of one issue.

use catalogue_core::{Catalogue, DataSources, Date, Diagnostics, Tier};

use super::notes::{approvers_section, demotion_section, not_checked_section, submit_section};
use super::report::{Block, CheckReport, Section};
use super::validation::validation_section;
use crate::convert::{
    IssueMeta, JudgmentDraft, Submission, draft_toml, entry_toml, judgment_draft, submission,
};
use crate::form::{Form, FormError, FormKind};

/// Everything a check needs. The caller reads the files and the GitHub API.
#[derive(Debug, Clone, Copy)]
pub struct CheckInput<'a> {
    /// The data files on `main`.
    pub main: &'a DataSources,
    /// The form that the issue label names.
    pub kind: FormKind,
    /// The parsed issue body.
    pub form: &'a Result<Form, Vec<FormError>>,
    /// The issue number and author.
    pub meta: &'a IssueMeta,
    /// The date that becomes `added` on a new entry.
    pub today: Date,
    /// The entry's current tier, for a demotion. `None` when unknown.
    pub current_tier: Option<Tier>,
}

/// The record an issue proposes.
pub(crate) enum Record {
    /// A new entry.
    Submit(Box<Submission>),
    /// A judgment that waits for approval.
    Judgment(JudgmentDraft),
}

impl Record {
    /// The file path relative to the repository root.
    pub(crate) fn path(&self) -> String {
        match self {
            Self::Submit(found) => found.path(),
            Self::Judgment(draft) => draft.path(),
        }
    }

    /// The file text, or the serializer error.
    pub(super) fn text(&self) -> Result<String, String> {
        let text = match self {
            Self::Submit(found) => entry_toml(&found.entry),
            Self::Judgment(draft) => draft_toml(draft),
        };
        text.map_err(|error| error.to_string())
    }
}

/// What the sections share: the input, the main catalogue and the record.
pub(super) struct Context<'a> {
    /// The check input.
    pub input: &'a CheckInput<'a>,
    /// The catalogue on `main`, or every error that stops it loading.
    pub main: Result<Catalogue, Diagnostics>,
    /// The record, when the form parses and converts.
    pub record: Option<Record>,
}

/// Checks one issue and builds its report.
///
/// The sections come in a fixed order: form, parse problems, file, validation,
/// then the submission or demotion facts, approvers, and what the check skips.
/// Lists are sorted. The report holds no timestamp.
pub fn check_submission(input: &CheckInput<'_>) -> CheckReport {
    let (record, problems) = convert(input);
    let context = Context {
        input,
        main: Catalogue::load(input.main.clone()),
        record,
    };
    let mut blocking = !problems.is_empty();
    let mut sections = vec![form_section(input)];
    sections.push(if problems.is_empty() {
        text_section("Parse problems", "None.")
    } else {
        Section {
            title: "Parse problems",
            blocks: vec![Block::Quoted(problems)],
        }
    });
    let text = context.record.as_ref().map(Record::text);
    sections.push(file_section(&context, text.as_ref(), &mut blocking));
    sections.push(match &text {
        Some(Ok(text)) => validation_section(&context, text, &mut blocking),
        Some(Err(_)) => text_section("Validation", "Not run. The file does not serialize."),
        None => text_section("Validation", "Not run. The form must parse first."),
    });
    if input.kind == FormKind::Submit {
        sections.push(submit_section(&context));
    }
    if input.kind == FormKind::Demotion {
        sections.push(demotion_section(input.current_tier, &mut blocking));
    }
    sections.push(approvers_section(&context));
    sections.push(not_checked_section(input.kind));
    CheckReport { blocking, sections }
}

/// The record of the parsed form, and every problem in the order found.
fn convert(input: &CheckInput<'_>) -> (Option<Record>, Vec<String>) {
    proposed_record(input.form, input.kind, input.meta, input.today)
}

/// The record that `form` proposes under the label `kind`, and every problem
/// in the order found. `today` becomes `added` on a new entry.
pub(crate) fn proposed_record(
    form: &Result<Form, Vec<FormError>>,
    kind: FormKind,
    meta: &IssueMeta,
    today: Date,
) -> (Option<Record>, Vec<String>) {
    let form = match form {
        Ok(form) => form,
        Err(errors) => return (None, errors.iter().map(ToString::to_string).collect()),
    };
    if form.kind() != kind {
        let problem = format!(
            "the body holds the `{}` form, but the label names `{}`",
            form.kind(),
            kind
        );
        return (None, vec![problem]);
    }
    let record = match form {
        Form::Submit(answers) => {
            submission(answers, meta, today).map(|found| Record::Submit(Box::new(found)))
        }
        Form::Judgment(answers) => judgment_draft(answers, meta).map(Record::Judgment),
    };
    match record {
        Ok(record) => (Some(record), Vec::new()),
        Err(errors) => (None, errors.iter().map(ToString::to_string).collect()),
    }
}

/// A section that holds one line of bot text.
pub(super) fn text_section(title: &'static str, text: &str) -> Section {
    Section {
        title,
        blocks: vec![Block::Text(text.to_owned())],
    }
}

fn form_section(input: &CheckInput<'_>) -> Section {
    text_section(
        "Form",
        &format!(
            "Form: `{}`. Issue: {}. Reporter: `{}`.",
            input.kind, input.meta.number, input.meta.author
        ),
    )
}

fn file_section(
    context: &Context<'_>,
    text: Option<&Result<String, String>>,
    blocking: &mut bool,
) -> Section {
    let (Some(record), Some(text)) = (&context.record, text) else {
        return text_section("File", "No file. Fix the parse problems first.");
    };
    let mut lead = format!("Path: `{}`.", record.path());
    if matches!(record, Record::Judgment(_)) {
        lead.push_str(" The approver and the date are pending. The bot sets them at approval.");
    }
    let body = match text {
        Ok(text) => Block::Code {
            lang: "toml",
            content: text.clone(),
        },
        Err(error) => {
            *blocking = true;
            Block::Quoted(vec![format!("the file does not serialize: {error}")])
        }
    };
    Section {
        title: "File",
        blocks: vec![Block::Text(lead), body],
    }
}
