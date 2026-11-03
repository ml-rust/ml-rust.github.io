//! A `submit` form into the entry it proposes and the notes for the data PR.

use catalogue_core::{
    CrateName, Date, Description, DescriptionSource, Distribution, Entry, EntryKind, GithubLogin,
    Slug, description_text, entry_name, repo_link,
};
use url::Url;

use super::meta::{ENTRIES_DIR, IssueMeta};
use crate::form::{AFFILIATED, FormError, NOT_AFFILIATED, SUBMIT, SubmitForm, excerpt, label};

/// The reporter's answer to the affiliation question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affiliation {
    /// "I am not affiliated with this project."
    NotAffiliated,
    /// "I maintain or work on this project."
    Affiliated,
}

impl Affiliation {
    /// The form sentence for this answer.
    pub fn sentence(self) -> &'static str {
        match self {
            Self::NotAffiliated => NOT_AFFILIATED,
            Self::Affiliated => AFFILIATED,
        }
    }

    /// The answer whose sentence is exactly `text`.
    pub fn from_sentence(text: &str) -> Option<Self> {
        [Self::NotAffiliated, Self::Affiliated]
            .into_iter()
            .find(|answer| answer.sentence() == text)
    }
}

/// Facts for the data PR body. None of them goes into the entry file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrNotes {
    /// Links to projects, posts or packages that use the project.
    pub evidence_of_use: Vec<Url>,
    /// The reporter's affiliation answer.
    pub affiliation: Affiliation,
    /// The issue author.
    pub reporter: GithubLogin,
    /// The issue link.
    pub issue: Url,
}

/// Where the slug of a submission comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlugSource {
    /// The first crate in `Crates`, whose name the repository name matches.
    PrimaryCrate,
    /// The repository name.
    RepositoryName,
}

/// The entry a `submit` form proposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    /// The entry slug, which names the file.
    pub slug: Slug,
    /// Where the slug comes from.
    pub slug_source: SlugSource,
    /// The entry.
    pub entry: Entry,
    /// The facts for the data PR body.
    pub notes: PrNotes,
}

impl Submission {
    /// The entry file path relative to the repository root: `data/entries/<slug>.toml`.
    pub fn path(&self) -> String {
        format!("{ENTRIES_DIR}/{}.toml", self.slug)
    }
}

/// Collects the problems of every field while the conversion goes on.
#[derive(Default)]
struct Problems(Vec<FormError>);

impl Problems {
    fn take<T>(&mut self, id: &'static str, result: Result<T, String>) -> Option<T> {
        result
            .map_err(|reason| {
                self.0.push(FormError::Invalid {
                    label: label(SUBMIT, id),
                    reason,
                });
            })
            .ok()
    }

    fn all<T>(&mut self, id: &'static str, results: Vec<Result<T, String>>) -> Vec<T> {
        results
            .into_iter()
            .filter_map(|result| self.take(id, result))
            .collect()
    }
}

/// The link in `text`, or the reason it is not one.
fn link(text: &str) -> Result<Url, String> {
    Url::parse(text).map_err(|_| format!("`{}` is not a valid link", excerpt(text)))
}

/// The slug and its source, from the first crate or else the repository name.
fn derive_slug(crates: &[String], repo: &Url) -> Result<(Slug, SlugSource), String> {
    let segment = repo
        .path_segments()
        .and_then(|mut parts| parts.next_back())
        .unwrap_or_default();
    let primary = crates.first().map_or(segment, String::as_str);
    let (name, slug) = entry_name(primary, repo).map_err(|_| match crates.first() {
        Some(name) => format!("the first crate `{}` gives no slug", excerpt(name)),
        None => format!("the repository name `{segment}` gives no slug"),
    })?;
    let source = if crates.is_empty() || name != primary {
        SlugSource::RepositoryName
    } else {
        SlugSource::PrimaryCrate
    };
    Ok((slug, source))
}

/// Converts the answers of a `submit` form into the entry it proposes.
///
/// `today` becomes `added`. `distribution` is set when any install channel is
/// given, even for a library, so validation reports it. Fails with every
/// field problem found.
pub fn submission(
    form: &SubmitForm,
    meta: &IssueMeta,
    today: Date,
) -> Result<Submission, Vec<FormError>> {
    let mut problems = Problems::default();
    let kind = problems.take(
        "kind",
        match form.kind.as_str() {
            "library" => Ok(EntryKind::Library),
            "app" => Ok(EntryKind::App),
            other => Err(format!("`{}` is not a kind", excerpt(other))),
        },
    );
    let repo = problems.take("repo", repo_link(Some(form.repo.as_str())));
    let slug = repo
        .as_ref()
        .and_then(|(url, _)| problems.take("repo", derive_slug(&form.crates, url)));
    let crates_result: Vec<_> = form
        .crates
        .iter()
        .map(|name| CrateName::new(name.as_str()).map_err(|error| error.to_string()))
        .collect();
    let crates = problems.all("crates", crates_result);
    let topics_result: Vec<_> = form
        .topics
        .iter()
        .map(|topic| Slug::new(topic.as_str()).map_err(|error| error.to_string()))
        .collect();
    let topics = problems.all("topics", topics_result);
    let text = problems.take(
        "description",
        description_text(Some(form.description.as_str())),
    );
    let source = problems.take(
        "description_source",
        match form.description_source.as_str() {
            "readme" => Ok(DescriptionSource::Readme),
            "human" => Ok(DescriptionSource::Human),
            other => Err(format!("`{}` is not a source", excerpt(other))),
        },
    );
    let source_url = form
        .description_source_url
        .as_deref()
        .and_then(|raw| problems.take("description_source_url", link(raw)));
    let homepage = form
        .homepage
        .as_deref()
        .and_then(|raw| problems.take("homepage", link(raw)));
    let evidence_result: Vec<_> = form.evidence_of_use.iter().map(|raw| link(raw)).collect();
    let evidence_of_use = problems.all("evidence_of_use", evidence_result);
    let affiliation = problems.take(
        "affiliation",
        Affiliation::from_sentence(&form.affiliation)
            .ok_or_else(|| format!("`{}` is not an answer", excerpt(&form.affiliation))),
    );
    let issue = meta
        .issue_url()
        .map_err(|error| problems.0.push(error))
        .ok();

    let has_distribution =
        form.homebrew.is_some() || !form.docker.is_empty() || form.repology.is_some();
    let distribution = has_distribution.then(|| Distribution {
        homebrew: form.homebrew.clone(),
        docker: form.docker.clone(),
        repology: form.repology.clone(),
    });

    let (
        Some(kind),
        Some((repo, org_affiliated)),
        Some((slug, slug_source)),
        Some(text),
        Some(source),
        Some(affiliation),
        Some(issue),
    ) = (kind, repo, slug, text, source, affiliation, issue)
    else {
        return Err(problems.0);
    };
    if !problems.0.is_empty() {
        return Err(problems.0);
    }
    Ok(Submission {
        slug,
        slug_source,
        entry: Entry {
            name: form.name.clone(),
            kind,
            repo,
            homepage,
            crates,
            topics,
            description: Description {
                text,
                source,
                source_url,
            },
            org_affiliated,
            distribution,
            added: today,
        },
        notes: PrNotes {
            evidence_of_use,
            affiliation,
            reporter: meta.author.clone(),
            issue,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    fn meta() -> IssueMeta {
        IssueMeta {
            number: NonZeroU64::new(12).expect("non-zero"),
            author: GithubLogin::new("dave").expect("login"),
        }
    }

    fn today() -> Date {
        Date::new(2026, 11, 3).expect("date")
    }

    fn form() -> SubmitForm {
        SubmitForm {
            name: "Gizmo".to_owned(),
            kind: "library".to_owned(),
            repo: "https://github.com/Someone/Gizmo.git".to_owned(),
            crates: vec!["gizmo-core".to_owned(), "gizmo-nn".to_owned()],
            topics: vec!["ml".to_owned()],
            description: "Learns\n  from data.".to_owned(),
            description_source: "human".to_owned(),
            description_source_url: None,
            homepage: Some("https://gizmo.example.com".to_owned()),
            homebrew: None,
            docker: Vec::new(),
            repology: None,
            evidence_of_use: vec!["https://example.com/uses-gizmo".to_owned()],
            affiliation: NOT_AFFILIATED.to_owned(),
        }
    }

    #[test]
    fn a_library_converts() {
        let found = submission(&form(), &meta(), today()).expect("converts");
        assert_eq!(found.slug.as_str(), "gizmo");
        assert_eq!(found.slug_source, SlugSource::RepositoryName);
        assert_eq!(found.path(), "data/entries/gizmo.toml");
        let entry = &found.entry;
        assert_eq!(entry.name, "Gizmo");
        assert_eq!(entry.kind, EntryKind::Library);
        assert_eq!(entry.repo.as_str(), "https://github.com/someone/gizmo");
        assert!(!entry.org_affiliated);
        assert_eq!(entry.description.text, "Learns from data.");
        assert_eq!(entry.description.source, DescriptionSource::Human);
        assert_eq!(entry.distribution, None);
        assert_eq!(entry.added, today());
        assert!(entry.problems().is_empty(), "{:?}", entry.problems());
        assert_eq!(found.notes.affiliation, Affiliation::NotAffiliated);
        assert_eq!(found.notes.reporter.as_str(), "dave");
        assert_eq!(
            found.notes.issue.as_str(),
            "https://github.com/ml-rust/ml-rust.github.io/issues/12"
        );
        assert_eq!(found.notes.evidence_of_use.len(), 1);
    }

    #[test]
    fn the_primary_crate_names_the_slug_when_the_repo_matches() {
        let mut input = form();
        input.repo = "https://github.com/someone/gizmo-core".to_owned();
        let found = submission(&input, &meta(), today()).expect("converts");
        assert_eq!(found.slug.as_str(), "gizmo-core");
        assert_eq!(found.slug_source, SlugSource::PrimaryCrate);
    }

    #[test]
    fn an_org_repo_is_affiliated() {
        let mut input = form();
        input.repo = "https://github.com/ml-rust/gizmo".to_owned();
        let found = submission(&input, &meta(), today()).expect("converts");
        assert!(found.entry.org_affiliated);
    }

    #[test]
    fn an_app_without_crates_takes_the_repo_name() {
        let mut input = form();
        input.kind = "app".to_owned();
        input.crates.clear();
        input.homebrew = Some("gizmo".to_owned());
        let found = submission(&input, &meta(), today()).expect("converts");
        assert_eq!(found.slug.as_str(), "gizmo");
        assert_eq!(found.slug_source, SlugSource::RepositoryName);
        let distribution = found.entry.distribution.expect("distribution");
        assert_eq!(distribution.homebrew.as_deref(), Some("gizmo"));
    }

    #[test]
    fn a_library_keeps_its_distribution_for_validation() {
        let mut input = form();
        input.docker = vec!["someone/gizmo".to_owned()];
        let found = submission(&input, &meta(), today()).expect("converts");
        assert!(found.entry.distribution.is_some());
        assert!(
            found
                .entry
                .problems()
                .iter()
                .any(|p| p.contains("only an app can have `distribution`"))
        );
    }

    #[test]
    fn every_bad_field_is_reported() {
        let mut input = form();
        input.repo = "http://github.com/a/b".to_owned();
        input.crates = vec!["9bad".to_owned()];
        input.topics = vec!["Bad_Topic".to_owned()];
        input.description = "x".repeat(301);
        input.homepage = Some("not a link".to_owned());
        let errors = submission(&input, &meta(), today()).expect_err("problems");
        let labels: Vec<&str> = errors
            .iter()
            .filter_map(|error| match error {
                FormError::Invalid { label, .. } => Some(*label),
                _ => None,
            })
            .collect();
        assert_eq!(
            labels,
            [
                "Repository URL",
                "Crates",
                "Topics",
                "Description",
                "Homepage"
            ]
        );
    }

    #[test]
    fn affiliation_sentences_map_exactly() {
        assert_eq!(
            Affiliation::from_sentence(AFFILIATED),
            Some(Affiliation::Affiliated)
        );
        assert_eq!(
            Affiliation::from_sentence(NOT_AFFILIATED),
            Some(Affiliation::NotAffiliated)
        );
        assert_eq!(Affiliation::from_sentence("I maintain it."), None);
    }
}
