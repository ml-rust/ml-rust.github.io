//! The judgment and history context of one entry.

use catalogue_core::{Catalogue, Context, Date, JudgmentKind, Slug};

/// The context of entry `entry_slug` on `today`.
///
/// - `maintainer_statement`: the status of the latest `maintainer` judgment by
///   date. On the same date the higher issue number wins.
/// - `demotion_approved`: any `demotion` judgment exists for the entry.
/// - `featured_approved`: any `featured-approval` judgment exists for the entry.
/// - `org_affiliated`: the entry's own flag. False for a slug the catalogue lacks.
/// - `previous_tier` and `prior_dead_checks`: none and 0. Published history
///   does not exist yet, so every check reads as a first check.
pub fn context(entry_slug: &Slug, catalogue: &Catalogue, today: Date) -> Context {
    let mut context = Context::new(today);
    context.org_affiliated = catalogue
        .entry(entry_slug)
        .is_some_and(|entry| entry.org_affiliated);
    let mut latest = None;
    for (&issue, judgment) in catalogue.judgments() {
        if &judgment.entry != entry_slug {
            continue;
        }
        match &judgment.judgment {
            JudgmentKind::Maintainer { status } => {
                let key = (judgment.date, issue);
                if latest.is_none_or(|(found, _)| key > found) {
                    latest = Some((key, *status));
                }
            }
            JudgmentKind::Demotion { .. } => context.demotion_approved = true,
            JudgmentKind::FeaturedApproval {} => context.featured_approved = true,
            _ => {}
        }
    }
    context.maintainer_statement = latest.map(|(_, status)| status);
    context
}

#[cfg(test)]
mod tests {
    use catalogue_core::{DataSources, MaintainerStatus, SourceFile};

    use super::*;

    const ENTRY: &str = r#"name = "Alpha"
kind = "library"
repo = "https://github.com/ml-rust/alpha"
crates = ["alpha"]
topics = ["ml"]
org_affiliated = true
added = 2026-01-01

[description]
text = "A library."
source = "human"
"#;

    fn file(path: &str, contents: &str) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            contents: contents.to_owned(),
        }
    }

    fn judgment(issue: u32, date: &str, body: &str) -> SourceFile {
        let text = format!(
            "entry = \"alpha\"\nissue = \"https://github.com/ml-rust/ml-rust.github.io/issues/{issue}\"\n\
             reporter = \"dave\"\napprover = \"bob\"\ndate = {date}\n\
             evidence = [\"https://example.com/proof\"]\n\n[judgment]\n{body}"
        );
        file(&format!("judgments/{issue}.toml"), &text)
    }

    fn maintainer(issue: u32, date: &str, status: &str) -> SourceFile {
        judgment(
            issue,
            date,
            &format!("type = \"maintainer\"\nstatus = \"{status}\"\n"),
        )
    }

    fn catalogue(judgments: Vec<SourceFile>) -> Catalogue {
        let sources = DataSources {
            entries: vec![
                file("entries/alpha.toml", ENTRY),
                file("entries/beta.toml", &ENTRY.replace("\"alpha\"", "\"beta\"")),
            ],
            judgments,
            maintainers: Some(file(
                "maintainers.toml",
                "[[maintainer]]\nlogin = \"bob\"\norg_member = false\nsince = 2026-01-01\n",
            )),
            topics: Some(file(
                "topics.toml",
                "[[topic]]\nid = \"ml\"\nname = \"ML\"\n",
            )),
            ..DataSources::default()
        };
        Catalogue::load(sources).expect("valid data")
    }

    fn slug(text: &str) -> Slug {
        Slug::new(text).expect("slug")
    }

    fn today() -> Date {
        Date::new(2026, 11, 3).expect("date")
    }

    #[test]
    fn the_latest_maintainer_statement_by_date_wins() {
        let found = catalogue(vec![
            maintainer(1, "2026-03-01", "maintained"),
            maintainer(2, "2026-05-01", "finished"),
            maintainer(3, "2026-04-01", "maintained"),
        ]);
        let ctx = context(&slug("alpha"), &found, today());
        assert_eq!(ctx.maintainer_statement, Some(MaintainerStatus::Finished));
        assert_eq!(ctx.today, today());
        let tied = catalogue(vec![
            maintainer(4, "2026-05-01", "maintained"),
            maintainer(5, "2026-05-01", "finished"),
        ]);
        let ctx = context(&slug("alpha"), &tied, today());
        assert_eq!(ctx.maintainer_statement, Some(MaintainerStatus::Finished));
    }

    #[test]
    fn approvals_and_the_org_flag_apply_to_their_entry_only() {
        let found = catalogue(vec![
            judgment(
                1,
                "2026-03-01",
                "type = \"demotion\"\nfrom = \"featured\"\nto = \"listed\"\n",
            ),
            judgment(2, "2026-03-02", "type = \"featured-approval\"\n"),
        ]);
        let alpha = context(&slug("alpha"), &found, today());
        assert!(alpha.demotion_approved);
        assert!(alpha.featured_approved);
        assert!(alpha.org_affiliated);
        assert_eq!(alpha.maintainer_statement, None);
        assert_eq!(alpha.previous_tier, None);
        assert_eq!(alpha.prior_dead_checks, 0);
        let beta = context(&slug("beta"), &found, today());
        assert!(!beta.demotion_approved && !beta.featured_approved);
        assert!(beta.org_affiliated);
        assert!(!context(&slug("ghost"), &found, today()).org_affiliated);
    }
}
