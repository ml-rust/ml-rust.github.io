//! The link check of one issue: fetch each page, ask a model, check the quote.

use catalogue_bot::Sha256;
use catalogue_bot::triage::{
    Claim, LinkClass, LinkResult, Outcome, anchor, classify, page_excerpt, parse_answer, prompt,
    triage_digest,
};
use catalogue_github::RepoClient;
use url::Url;

use crate::client::Llm;
use crate::error::TriageError;
use crate::fetch::{PageError, WebFetch, github_page};

/// The result of checking every link of one issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriageResult {
    /// The digest of the claim and the links, from [`triage_digest`].
    pub digest: Sha256,
    /// One result per link, in link order.
    pub lines: Vec<LinkResult>,
    /// Whether at least one page reached the models and no model answered for any.
    ///
    /// The caller then keeps the previous section and checks again later.
    pub all_failed: bool,
}

/// The readers and the model client one check uses.
pub struct Checkers<'c, 'g, 'w, 'l> {
    /// Reads GitHub pages with the bot's installation token.
    pub github: &'c mut RepoClient<'g>,
    /// Reads every other page through Ollama `web_fetch`.
    pub web: &'c mut WebFetch<'w>,
    /// Asks the models.
    pub llm: &'c mut Llm<'l>,
}

/// Checks the first `max_links` of `links` against `claim`. Later links are not checked.
///
/// One link failing never stops the others. A page that cannot be read gives
/// [`Outcome::Unreachable`]. A page no model answers for gives [`Outcome::NoAnswer`].
///
/// # Errors
///
/// Returns [`TriageError::NoProvider`] when no model provider is set. Nothing is fetched then.
pub fn triage(
    claim: &Claim,
    links: &[Url],
    max_links: usize,
    checkers: &mut Checkers<'_, '_, '_, '_>,
) -> Result<TriageResult, TriageError> {
    if checkers.llm.providers().is_empty() {
        return Err(TriageError::NoProvider);
    }
    let mut asked = 0_usize;
    let mut answered = 0_usize;
    let mut lines = Vec::with_capacity(links.len());
    for (index, url) in links.iter().enumerate() {
        let outcome = if index >= max_links {
            Outcome::NotChecked
        } else {
            match page_text(url, checkers) {
                Err(error) => Outcome::Unreachable(error.to_string()),
                Ok(text) => {
                    asked = asked.saturating_add(1);
                    let request = prompt(claim, url, &text);
                    match checkers.llm.ask(&request, parse_answer) {
                        Ok(asked_model) => {
                            answered = answered.saturating_add(1);
                            Outcome::Checked(anchor(&asked_model.answer, page_excerpt(&text)))
                        }
                        Err(TriageError::NoProvider) => return Err(TriageError::NoProvider),
                        Err(TriageError::AllFailed { .. }) => Outcome::NoAnswer,
                    }
                }
            }
        };
        lines.push(LinkResult {
            url: url.clone(),
            outcome,
        });
    }
    Ok(TriageResult {
        digest: triage_digest(claim, links),
        lines,
        all_failed: asked > 0 && answered == 0,
    })
}

/// The text of the page at `url`, from the reader its class names.
fn page_text(url: &Url, checkers: &mut Checkers<'_, '_, '_, '_>) -> Result<String, PageError> {
    let text = match classify(url) {
        LinkClass::GitHub(target) => github_page(checkers.github, &target)?,
        LinkClass::Other => checkers.web.page(url)?,
        LinkClass::Unsupported => return Err(PageError::Link),
    };
    if text.trim().is_empty() {
        return Err(PageError::Empty);
    }
    Ok(text)
}
