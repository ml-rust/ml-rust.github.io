//! Writes one state report in steps with one purpose each: shortlist the READMEs worth reading,
//! read, write, pick charts, combine, audit, and fix when the audit finds an error. Code does the
//! combining. Every step tries the models in priority order.

use std::cell::RefCell;
use std::time::Instant;

use catalogue_bot::triage::{AnswerError, Prompt};
use catalogue_core::{Date, STATE_SCHEMA, Slug, StateReport};
use catalogue_triage::{Asked, AttemptFailure, Llm, TriageError};
use thiserror::Error;

use crate::check::{
    parse_charts, parse_notes, parse_review, parse_shortlist, parse_summary, read_summary,
    summary_errors, unjudged,
};
use crate::facts::Projects;
use crate::needs::need_list;
use crate::prompt::{
    RULE_ERROR, audit_prompt, chart_prompt, fix_prompt, judge_prompt, read_prompt,
    shortlist_prompt, write_prompt,
};

/// The most READMEs one report reads in full. Summaries normally leave no need to.
pub const MAX_READMES: usize = 30;

/// A report that could not be written. Nothing replaces the last report then.
#[derive(Debug, Error)]
#[error("no model finished the `{step}` step: {source}. The last report stays. Rerun later")]
pub struct WriteError {
    /// The step no model finished.
    pub step: &'static str,
    /// What the models answered.
    pub source: TriageError,
}

/// Runs each step and keeps the log of what it took.
struct Run<'l> {
    log: RefCell<&'l mut dyn FnMut(&str)>,
}

impl Run<'_> {
    fn say(&self, line: &str) {
        (*self.log.borrow_mut())(line);
    }

    /// Asks `llm` until an answer passes `parse`, and logs the step.
    fn step<T>(
        &self,
        llm: &mut Llm<'_>,
        step: &'static str,
        prompt: &Prompt,
        parse: &dyn Fn(&str) -> Result<T, String>,
    ) -> Result<Asked<T>, WriteError> {
        let started = Instant::now();
        let asked = llm
            .ask(prompt, |content| {
                parse(content).map_err(|reason| {
                    self.say(&format!("{step}: rejected: {reason}"));
                    AnswerError::Rejected
                })
            })
            .map_err(|source| WriteError { step, source })?;
        let seconds = started.elapsed().as_secs();
        self.say(&format!("{step}: done by {} in {seconds}s", asked.model));
        for failure in &asked.failed {
            self.note_failure(step, failure);
        }
        Ok(asked)
    }

    fn note_failure(&self, step: &str, failure: &AttemptFailure) {
        self.say(&format!("{step}: failed attempt: {failure}"));
    }
}

/// Writes a full report over `projects`, or fails with nothing written.
///
/// `log` receives one line per finished step, per rejected answer and per audit error.
///
/// # Errors
///
/// Returns a [`WriteError`] when no model gives a valid answer for one step.
pub fn write_report(
    llm: &mut Llm<'_>,
    projects: &Projects,
    written: Date,
    fetch_readme: &dyn Fn(&Slug) -> Option<String>,
    log: &mut dyn FnMut(&str),
) -> Result<StateReport, WriteError> {
    let run = Run {
        log: RefCell::new(log),
    };
    let list = need_list();
    let shortlist = run
        .step(
            llm,
            "shortlist",
            &shortlist_prompt(&list, projects, MAX_READMES),
            &|content| parse_shortlist(content, projects, MAX_READMES),
        )?
        .answer;
    let readmes: Vec<(Slug, String)> = shortlist
        .iter()
        .filter_map(|slug| fetch_readme(slug).map(|text| (slug.clone(), text)))
        .collect();
    run.say(&format!(
        "shortlist: read {} of {} READMEs",
        readmes.len(),
        shortlist.len()
    ));
    let mut notes = run
        .step(
            llm,
            "read",
            &read_prompt(&list, projects, &readmes),
            &parse_notes,
        )?
        .answer;
    let missing = unjudged(&notes, &list, projects);
    if !missing.is_empty() {
        run.say(&format!("read: {} rows left to judge", missing.len()));
        let late = run
            .step(
                llm,
                "judge",
                &judge_prompt(&missing, projects),
                &parse_notes,
            )?
            .answer;
        notes.push_str("\n\n");
        notes.push_str(&late);
    }
    let request = write_prompt(&list, &notes, projects);
    // The draft stays in the words the model wrote, so a fix keeps the reply shape it was asked for.
    let draft = RefCell::new(String::new());
    let read_draft = |content: &str| {
        let summary = read_summary(content)?;
        draft.replace(content.to_owned());
        Ok(summary)
    };
    let first = run.step(llm, "write", &request, &read_draft)?.answer;
    let charts = run
        .step(llm, "charts", &chart_prompt(projects), &|content| {
            parse_charts(content, projects)
        })?
        .answer;
    let written_draft = draft.borrow().clone();
    let found = run
        .step(
            llm,
            "audit",
            &audit_prompt(&written_draft, &notes, projects, &readmes),
            &parse_review,
        )?
        .answer;
    let mut errors: Vec<String> = summary_errors(&first, &list, projects)
        .into_iter()
        .map(|error| format!("{RULE_ERROR} {error}"))
        .collect();
    errors.extend(found);
    let summary = if errors.is_empty() {
        first
    } else {
        for error in &errors {
            run.say(&format!("audit: error: {error}"));
        }
        let fix = fix_prompt(&request, &written_draft, &errors, projects);
        let parse = |content: &str| parse_summary(content, &list, projects);
        run.step(llm, "fix", &fix, &parse)?.answer
    };
    Ok(StateReport {
        schema: STATE_SCHEMA,
        written,
        summary,
        charts,
    })
}
