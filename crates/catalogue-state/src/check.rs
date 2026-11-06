//! The checks on every model answer. A report that fails one never reaches the site.

use catalogue_core::{
    BuildPath, ChartNote, FilledHole, JobState, OpenHole, Readiness, Slug, StateSummary, ToolPick,
};
use serde::Deserialize;

use crate::facts::Projects;
use crate::needs::NeedList;

/// Words that bring popularity into the report. `ranking` and `downloads` stay out of this list,
/// because search ranking and model downloads are plain ML terms.
const RANKING_WORDS: [&str; 8] = [
    "star",
    "stars",
    "stargazers",
    "popular",
    "popularity",
    "trending",
    "tier",
    "featured",
];

/// Words that bring hype into the report.
const HYPE_WORDS: [&str; 5] = ["robust", "seamless", "powerful", "blazing", "revolutionary"];

/// The notes of the reading step, from model `content`. They must be one JSON object.
///
/// # Errors
///
/// Returns what breaks the shape.
pub fn parse_notes(content: &str) -> Result<String, String> {
    let body = unfenced(content);
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("JSON: {error}"))?;
    if value.is_object() {
        Ok(body.to_owned())
    } else {
        Err("the notes are not one JSON object".to_owned())
    }
}

/// Each job of `list` and project the reading step owes a judgment and left out of `notes`.
pub(crate) fn unjudged(notes: &str, list: &NeedList, projects: &Projects) -> Vec<(Slug, Slug)> {
    let value: serde_json::Value = serde_json::from_str(notes).unwrap_or_default();
    let judged = |topic: &Slug, slug: &Slug| {
        value["jobs"].as_array().is_some_and(|jobs| {
            jobs.iter()
                .filter(|job| job["topic"].as_str() == Some(topic.as_str()))
                .filter_map(|job| job["considered"].as_array())
                .flatten()
                .any(|row| row["slug"].as_str() == Some(slug.as_str()))
        })
    };
    let mut missing = Vec::new();
    for topic in &list.topic {
        for slug in projects.to_judge(&topic.id) {
            if !judged(&topic.id, slug) {
                missing.push((topic.id.clone(), slug.clone()));
            }
        }
    }
    missing
}

/// The READMEs a first pass asks for, from model `content`, up to `most`. An unknown slug is left out.
///
/// # Errors
///
/// Returns what breaks the shape. Asking for no README is a valid answer.
pub fn parse_shortlist(
    content: &str,
    projects: &Projects,
    most: usize,
) -> Result<Vec<Slug>, String> {
    #[derive(Deserialize)]
    struct RawShortlist {
        readmes: Vec<String>,
    }
    let raw: RawShortlist =
        serde_json::from_str(unfenced(content)).map_err(|e| format!("JSON: {e}"))?;
    let mut found: Vec<Slug> = Vec::new();
    for text in raw.readmes {
        if let Ok(slug) = Slug::new(text)
            && projects.get(&slug).is_some()
            && !found.contains(&slug)
            && found.len() < most
        {
            found.push(slug);
        }
    }
    Ok(found)
}

/// The words of the report from model `content`.
///
/// # Errors
///
/// Returns what breaks a rule, naming the field.
pub fn parse_summary(
    content: &str,
    list: &NeedList,
    projects: &Projects,
) -> Result<StateSummary, String> {
    let raw = read_summary(content)?;
    let errors = summary_errors(&raw, list, projects);
    if errors.is_empty() {
        Ok(raw)
    } else {
        Err(errors.join("; "))
    }
}

/// The words of the report from model `content`, with its rules still unchecked.
///
/// # Errors
///
/// Returns what breaks the reply shape.
pub(crate) fn read_summary(content: &str) -> Result<StateSummary, String> {
    serde_json::from_str(unfenced(content)).map_err(|e| format!("JSON: {e}"))
}

/// Every rule `summary` breaks, naming the field of each.
pub(crate) fn summary_errors(
    summary: &StateSummary,
    list: &NeedList,
    projects: &Projects,
) -> Vec<String> {
    let check = Checker { projects };
    let mut errors = Vec::new();
    errors.extend(check.text("headline", &summary.headline, 170).err());
    errors.extend(check.text("summary", &summary.summary, 1000).err());
    if summary.jobs.len() != list.topic.len() {
        errors.push(format!(
            "jobs: {} rows for {} jobs",
            summary.jobs.len(),
            list.topic.len()
        ));
    }
    for topic in &list.topic {
        match summary.jobs.iter().find(|job| job.topic == topic.id) {
            Some(job) => check.job(job, &mut errors),
            None => errors.push(format!("jobs: no row for `{}`", topic.id)),
        }
    }
    errors.extend(check.count("use_now", summary.use_now.len(), 5, 12).err());
    for pick in &summary.use_now {
        check.pick(pick, &mut errors);
    }
    errors.extend(check.count("watch", summary.watch.len(), 2, 6).err());
    for pick in &summary.watch {
        check.pick(pick, &mut errors);
    }
    let paths = summary.build_today.len();
    errors.extend(check.count("build_today", paths, 3, 5).err());
    for path in &summary.build_today {
        check.path(path, &mut errors);
    }
    let filled = summary.holes_filled.len();
    errors.extend(check.count("holes_filled", filled, 3, 8).err());
    for hole in &summary.holes_filled {
        check.filled(hole, &mut errors);
    }
    let open = summary.holes_open.len();
    errors.extend(check.count("holes_open", open, 3, 8).err());
    for hole in &summary.holes_open {
        check.open(hole, &mut errors);
    }
    errors
}

/// The picked charts from model `content`.
///
/// # Errors
///
/// Returns what breaks a rule, naming the field.
pub fn parse_charts(content: &str, projects: &Projects) -> Result<Vec<ChartNote>, String> {
    #[derive(Deserialize)]
    struct RawCharts {
        charts: Vec<ChartNote>,
    }
    let raw: RawCharts =
        serde_json::from_str(unfenced(content)).map_err(|e| format!("JSON: {e}"))?;
    let check = Checker { projects };
    check.count("charts", raw.charts.len(), 2, 3)?;
    for (index, note) in raw.charts.iter().enumerate() {
        check.text("takeaway", &note.takeaway, 280)?;
        if raw.charts[..index]
            .iter()
            .any(|earlier| earlier.chart == note.chart)
        {
            return Err("charts: one chart is picked twice".to_owned());
        }
    }
    Ok(raw.charts)
}

/// The factual errors an audit lists from model `content`.
///
/// # Errors
///
/// Returns what breaks the reply shape.
pub fn parse_review(content: &str) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct RawReview {
        errors: Vec<String>,
    }
    let raw: RawReview =
        serde_json::from_str(unfenced(content)).map_err(|e| format!("JSON: {e}"))?;
    Ok(raw.errors)
}

/// `content` without surrounding whitespace and one optional code fence.
fn unfenced(content: &str) -> &str {
    let trimmed = content.trim();
    let Some(rest) = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
    else {
        return trimmed;
    };
    rest.strip_suffix("```").unwrap_or(rest).trim()
}

/// `text` with each dot between a letter and a digit joined, so H.264 reads as a name and no
/// number. The dot in v0.3 or pre-1.0 has a digit before it, so those still read as numbers.
fn with_standard_names_joined(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    chars
        .iter()
        .enumerate()
        .map(|(index, &found)| {
            let after_letter = index
                .checked_sub(1)
                .and_then(|before| chars.get(before))
                .is_some_and(char::is_ascii_alphabetic);
            let before_digit = chars
                .get(index.saturating_add(1))
                .is_some_and(char::is_ascii_digit);
            if found == '.' && after_letter && before_digit {
                '_'
            } else {
                found
            }
        })
        .collect()
}

struct Checker<'a> {
    projects: &'a Projects,
}

impl Checker<'_> {
    fn count(&self, field: &str, found: usize, least: usize, most: usize) -> Result<(), String> {
        if (least..=most).contains(&found) {
            Ok(())
        } else {
            Err(format!("{field}: {found} items, wanted {least} to {most}"))
        }
    }

    fn job(&self, job: &JobState, errors: &mut Vec<String>) {
        let field = format!("jobs.{}", job.topic);
        errors.extend(self.text(&field, &job.verdict, 400).err());
        let least = usize::from(job.readiness != Readiness::Early);
        let tools = self.count(&format!("{field}.tools"), job.tools.len(), least, 12);
        errors.extend(tools.err());
        for slug in &job.tools {
            errors.extend(self.maintained(&field, slug).err());
        }
    }

    fn pick(&self, pick: &ToolPick, errors: &mut Vec<String>) {
        errors.extend(self.maintained("pick", &pick.project).err());
        errors.extend(self.text("use_for", &pick.use_for, 130).err());
        errors.extend(self.text("why", &pick.why, 300).err());
    }

    fn path(&self, path: &BuildPath, errors: &mut Vec<String>) {
        errors.extend(self.text("goal", &path.goal, 130).err());
        errors.extend(self.text("why", &path.why, 320).err());
        errors.extend(self.text("watch_out", &path.watch_out, 280).err());
        errors.extend(self.count("stack", path.stack.len(), 2, 6).err());
        for slug in &path.stack {
            errors.extend(self.maintained("stack", slug).err());
        }
    }

    fn filled(&self, hole: &FilledHole, errors: &mut Vec<String>) {
        errors.extend(self.text("hole", &hole.hole, 160).err());
        errors.extend(self.text("note", &hole.note, 300).err());
        errors.extend(self.count("filled_by", hole.filled_by.len(), 1, 4).err());
        for slug in &hole.filled_by {
            errors.extend(self.maintained("filled_by", slug).err());
        }
    }

    fn open(&self, hole: &OpenHole, errors: &mut Vec<String>) {
        errors.extend(self.text("hole", &hole.hole, 160).err());
        errors.extend(self.text("impact", &hole.impact, 300).err());
        errors.extend(self.text("workaround", &hole.workaround, 300).err());
    }

    fn known(&self, field: &str, slug: &Slug) -> Result<(), String> {
        if self.projects.get(slug).is_some() {
            Ok(())
        } else {
            Err(format!("{field}: `{slug}` is not in the catalogue"))
        }
    }

    fn maintained(&self, field: &str, slug: &Slug) -> Result<(), String> {
        self.known(field, slug)?;
        if self.projects.get(slug).is_some_and(|found| found.active) {
            Ok(())
        } else {
            Err(format!("{field}: `{slug}` is not maintained"))
        }
    }

    /// Checks one piece of prose: its length, its markup, its numbers, its words and its links.
    fn text(&self, field: &str, text: &str, max: usize) -> Result<(), String> {
        if text.trim().is_empty() {
            return Err(format!("{field}: empty"));
        }
        if text.chars().count() > max {
            return Err(format!("{field}: longer than {max} characters"));
        }
        let lower = text.to_lowercase();
        for banned in ["http://", "https://", "www.", "<", ">", "`", "**", "\n"] {
            if lower.contains(banned) {
                return Err(format!("{field}: holds `{}`", banned.escape_debug()));
            }
        }
        let plain = with_standard_names_joined(&self.without_mentions(field, text)?);
        for word in plain.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-')) {
            if !word.is_empty() && word.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("{field}: holds the number `{word}` in: {text}"));
            }
            let word = word.to_lowercase();
            if RANKING_WORDS.contains(&word.as_str()) || HYPE_WORDS.contains(&word.as_str()) {
                return Err(format!("{field}: holds the word `{word}` in: {text}"));
            }
        }
        Ok(())
    }

    /// `text` without its `[[slug]]` mentions. Every mention must name a catalogue project.
    fn without_mentions(&self, field: &str, text: &str) -> Result<String, String> {
        let mut plain = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start) = rest.find("[[") {
            plain.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let end = after
                .find("]]")
                .ok_or_else(|| format!("{field}: a mention never closes"))?;
            let slug = Slug::new(after[..end].to_owned())
                .map_err(|_| format!("{field}: `{}` is no slug", &after[..end]))?;
            self.known(field, &slug)?;
            plain.push(' ');
            rest = &after[end + 2..];
        }
        plain.push_str(rest);
        if plain.contains('[') || plain.contains(']') {
            return Err(format!("{field}: holds a stray bracket"));
        }
        Ok(plain)
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::Date;
    use serde_json::{Value, json};

    use super::*;
    use crate::needs::{Need, TopicNeeds};

    fn projects() -> Projects {
        let file = json!({"entries": [
            {"slug": "tool", "name": "tool", "kind": "library", "topics": ["run-models"],
             "category": null, "description": {"text": "Runs"}, "tier": "listed", "native_word": null,
             "signals": [{"name": "last_release", "value": "2026-10-01"}]},
            {"slug": "old", "name": "old", "kind": "library", "topics": ["run-models"],
             "category": null, "description": {"text": "Old"}, "tier": "listed", "native_word": null,
             "signals": [{"name": "last_release", "value": "2019-10-01"}]},
        ]});
        Projects::parse(
            file.to_string().as_bytes(),
            Date::new(2026, 11, 6).expect("date"),
        )
        .expect("file")
    }

    fn list() -> NeedList {
        NeedList {
            topic: vec![TopicNeeds {
                id: Slug::new("run-models".to_owned()).expect("slug"),
                job: "Run models".to_owned(),
                needs: vec![Need {
                    id: "local".to_owned(),
                    ask: "Run one locally".to_owned(),
                }],
            }],
        }
    }

    fn answer() -> Value {
        let pick = json!({"project": "tool", "use_for": "Serving", "why": "It works."});
        let path = json!({"goal": "A server", "stack": ["tool", "tool"], "why": "It works.", "watch_out": "It wraps C++."});
        let filled =
            json!({"hole": "Tokenizers", "filled_by": ["tool"], "note": "No Python needed."});
        let open =
            json!({"hole": "No evals", "impact": "Teams stop.", "workaround": "Use Python."});
        json!({
            "headline": "Yes, for serving.",
            "summary": "Use [[tool]] to serve.",
            "jobs": [{"topic": "run-models", "readiness": "ready", "python": "none",
                      "verdict": "It works with [[tool]].", "tools": ["tool"]}],
            "use_now": [pick, pick, pick, pick, pick],
            "watch": [pick, pick],
            "build_today": [path, path, path],
            "holes_filled": [filled, filled, filled],
            "holes_open": [open, open, open],
        })
    }

    fn parse(value: &Value) -> Result<StateSummary, String> {
        parse_summary(&value.to_string(), &list(), &projects())
    }

    #[test]
    fn a_good_answer_passes_even_in_a_fence() {
        assert!(parse(&answer()).is_ok());
        let fenced = format!("```json\n{}\n```", answer());
        assert!(parse_summary(&fenced, &list(), &projects()).is_ok());
    }

    #[test]
    fn prose_with_a_number_a_link_markup_or_a_ranking_word_fails() {
        for (summary, needle) in [
            ("It has 500 users.", "number `500`"),
            ("Version 1.0 works.", "number `1`"),
            ("It is still pre-1.0 today.", "number `0` in: It is still"),
            ("Use v0.3 today.", "number `3`"),
            ("See https://example.com now.", "https://"),
            ("Use <b>this</b>.", "<"),
            ("It has many stars.", "`stars`"),
            ("It is very popular.", "`popular`"),
            ("A seamless choice.", "`seamless`"),
            ("Try [[nope]] instead.", "`nope`"),
            ("Try [[tool instead.", "never closes"),
        ] {
            let mut bad = answer();
            bad["summary"] = json!(summary);
            let error = parse(&bad).expect_err(summary);
            assert!(error.contains(needle), "{summary}: {error}");
        }
    }

    #[test]
    fn a_name_with_digits_is_no_number() {
        let mut good = answer();
        good["summary"] = json!("Use BM25, word2vec, x86_64, top-5 search, H.264 and 3D.");
        assert!(parse(&good).is_ok());
    }

    #[test]
    fn every_broken_rule_is_listed_at_once() {
        let mut bad = answer();
        bad["headline"] = json!("It has 500 users.");
        bad["summary"] = json!("It is very popular.");
        bad["use_now"][1]["project"] = json!("old");
        bad["holes_open"] = json!([]);
        let summary = read_summary(&bad.to_string()).expect("the shape holds");
        let errors = summary_errors(&summary, &list(), &projects());
        for needle in ["`500`", "`popular`", "`old`", "holes_open"] {
            assert!(
                errors.iter().any(|error| error.contains(needle)),
                "{needle}: {errors:?}"
            );
        }
    }

    #[test]
    fn a_job_needs_its_row_and_maintained_tools() {
        let mut bad = answer();
        bad["jobs"][0]["topic"] = json!("train-models");
        assert!(parse(&bad).expect_err("row").contains("no row"));
        let mut bad = answer();
        bad["jobs"][0]["tools"] = json!(["old"]);
        assert!(parse(&bad).expect_err("old").contains("not maintained"));
        let mut bad = answer();
        bad["jobs"][0]["tools"] = json!([]);
        assert!(parse(&bad).expect_err("empty").contains("tools"));
        bad["jobs"][0]["readiness"] = json!("early");
        assert!(parse(&bad).is_ok(), "an early job can name no tool");
    }

    #[test]
    fn a_shortlist_keeps_known_slugs_once_and_respects_the_cap() {
        let found = parse_shortlist(
            r#"{"readmes":["tool","nope","tool","Bad Slug","old"]}"#,
            &projects(),
            5,
        )
        .expect("shortlist");
        let names: Vec<&str> = found.iter().map(Slug::as_str).collect();
        assert_eq!(names, ["tool", "old"]);
        let capped = parse_shortlist(r#"{"readmes":["tool","old"]}"#, &projects(), 1);
        assert_eq!(capped.expect("capped").len(), 1);
        let none = parse_shortlist(r#"{"readmes":["nope"]}"#, &projects(), 5).expect("valid");
        assert!(none.is_empty(), "needing no README is a valid answer");
        assert!(parse_shortlist("nope", &projects(), 5).is_err());
    }

    #[test]
    fn a_pick_outside_the_catalogue_fails() {
        let mut bad = answer();
        bad["use_now"][0]["project"] = json!("nope");
        assert!(
            parse(&bad)
                .expect_err("pick")
                .contains("not in the catalogue")
        );
    }

    #[test]
    fn charts_come_from_the_menu_and_only_once() {
        let good = r#"{"charts":[{"chart":"engine","takeaway":"Most wrap C++."},{"chart":"gpu","takeaway":"CUDA leads."}]}"#;
        assert_eq!(parse_charts(good, &projects()).expect("good").len(), 2);
        let unknown =
            r#"{"charts":[{"chart":"pie","takeaway":"x"},{"chart":"gpu","takeaway":"y"}]}"#;
        assert!(parse_charts(unknown, &projects()).is_err());
        let twice = r#"{"charts":[{"chart":"gpu","takeaway":"x"},{"chart":"gpu","takeaway":"y"}]}"#;
        assert!(parse_charts(twice, &projects()).is_err());
    }

    #[test]
    fn a_project_the_notes_never_judged_is_listed() {
        let run = Slug::new("run-models".to_owned()).expect("slug");
        let tool = Slug::new("tool".to_owned()).expect("slug");
        let none = r#"{"jobs":[{"topic":"run-models","considered":[]}]}"#;
        assert_eq!(unjudged(none, &list(), &projects()), [(run, tool)]);
        let judged =
            r#"{"jobs":[{"topic":"run-models","considered":[{"slug":"tool","verdict":"fits"}]}]}"#;
        assert!(unjudged(judged, &list(), &projects()).is_empty());
    }

    #[test]
    fn notes_must_be_one_object_and_a_review_lists_errors() {
        assert!(parse_notes(r#"{"jobs":[]}"#).is_ok());
        assert!(parse_notes("[]").is_err());
        assert_eq!(parse_review(r#"{"errors":["x"]}"#).expect("review"), ["x"]);
        assert!(parse_review("nope").is_err());
    }
}
