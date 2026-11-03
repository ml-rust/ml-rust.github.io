//! The advisory LLM link check inside the poll pass.
//!
//! The check runs only when the claim, the links or the page sources change.
//! The page sources change when a `web_fetch` key is set or removed. An unchanged
//! check carries the section of the last bot comment forward. A check that no
//! model answers waits `retry_after_secs`, then runs again.

use catalogue_bot::triage::{
    Claim, evidence_links, section_of_comment, triage_digest, triage_section,
};
use catalogue_bot::{Form, Marker, Sha256};
use catalogue_github::{RepoClient, Secret};
use catalogue_signals::http::{Pacer, Transport};
use catalogue_triage::{Backoff, Checkers, Llm, Provider, WebFetch, triage};

use super::state::IssueState;
use crate::bot::config::TriageConfig;

/// Reads one environment variable by name. `None` when it is unset.
pub type EnvLookup<'a> = &'a dyn Fn(&str) -> Option<String>;

/// What the link check of one pass uses besides the GitHub client.
pub struct TriageDeps<'a> {
    /// Sends the model and `web_fetch` requests.
    pub transport: &'a dyn Transport,
    /// Spaces the model requests.
    pub llm_pacer: &'a mut dyn Pacer,
    /// Spaces the `web_fetch` requests.
    pub fetch_pacer: &'a mut dyn Pacer,
    /// Waits between two attempts at one model.
    pub backoff: &'a mut dyn Backoff,
    /// Reads the key variables the config names.
    pub env: EnvLookup<'a>,
}

/// The keys one pass found: the providers that have one, and the `web_fetch` key.
pub(super) struct Keys {
    providers: Vec<Provider>,
    web_key: Option<Secret>,
}

/// Reads the key of each provider and of `web_fetch` from `env`.
///
/// Returns `None` when the check is off or no provider has a key. Each
/// skipped provider adds one warning that names the provider and the
/// variable, never a value.
pub(super) fn read_keys(
    config: &TriageConfig,
    env: EnvLookup<'_>,
    warnings: &mut Vec<String>,
) -> Option<Keys> {
    if !config.enabled {
        return None;
    }
    let read = |name: &str| env(name).filter(|value| !value.trim().is_empty());
    let mut providers = Vec::with_capacity(config.providers.len());
    for found in &config.providers {
        let Some(key) = read(&found.api_key_env) else {
            warnings.push(format!(
                "link check provider `{}` is skipped: {} is unset or empty. Set it in the bot environment file",
                found.name, found.api_key_env
            ));
            continue;
        };
        match Provider::new(
            &found.name,
            &found.base_url,
            found.models.clone(),
            Secret::new(key),
            found.headers.clone(),
            found.request.clone(),
        ) {
            Ok(provider) => providers.push(provider),
            Err(error) => warnings.push(format!(
                "link check provider `{}` is skipped: {error}",
                found.name
            )),
        }
    }
    if providers.is_empty() {
        warnings.push(
            "the link check is off for this run: no provider has an API key. Set a provider key variable, then rerun"
                .to_owned(),
        );
        return None;
    }
    let web_key = config.fetch.as_ref().and_then(|fetch| {
        let key = read(&fetch.api_key_env);
        if key.is_none() {
            warnings.push(format!(
                "the link check reads GitHub pages only: {} is unset or empty. Set it in the bot environment file",
                fetch.api_key_env
            ));
        }
        key.map(Secret::new)
    });
    Some(Keys { providers, web_key })
}

/// The model client, the `web_fetch` reader and the limits of one pass.
pub(super) struct LinkCheck<'a> {
    llm: Llm<'a>,
    web: Option<WebFetch<'a>>,
    max_links: usize,
    retry_after_secs: u64,
}

impl<'a> LinkCheck<'a> {
    /// The check over `deps` with the providers and the key in `keys`.
    /// `None` when there are no keys, so the check is off.
    ///
    /// A `web_fetch` reader that cannot be built adds a warning, and the
    /// check then reads GitHub pages only.
    pub(super) fn start(
        deps: &'a mut TriageDeps<'_>,
        keys: Option<&'a Keys>,
        config: &TriageConfig,
        warnings: &mut Vec<String>,
    ) -> Option<Self> {
        let keys = keys?;
        let web = match (&config.fetch, &keys.web_key) {
            (Some(fetch), Some(key)) => {
                match WebFetch::new(
                    deps.transport,
                    &mut *deps.fetch_pacer,
                    &fetch.web_fetch_url,
                    key.clone(),
                ) {
                    Ok(reader) => Some(reader),
                    Err(error) => {
                        warnings.push(format!("the link check reads GitHub pages only: {error}"));
                        None
                    }
                }
            }
            _ => None,
        };
        Some(Self {
            llm: Llm::new(
                deps.transport,
                &mut *deps.llm_pacer,
                &mut *deps.backoff,
                &keys.providers,
                config.retry,
            ),
            web,
            max_links: config.max_links,
            retry_after_secs: config.retry_after_secs,
        })
    }
}

/// The last bot comment: its marker and its text.
pub(super) struct PreviousComment<'a> {
    /// The marker on its first line.
    pub marker: &'a Marker,
    /// The whole comment text.
    pub text: &'a str,
}

/// The link check part of one issue check.
#[derive(Debug, Default)]
pub(super) struct Triaged {
    /// The section the comment shows. `None` shows none.
    pub section: Option<String>,
    /// The digest the marker records.
    pub digest: Option<Sha256>,
    /// The unix second before which the check does not run again.
    pub retry_after: Option<i64>,
    /// Whether the models checked the links on this pass.
    pub ran: bool,
    /// Why the check failed on this pass. It holds no issue text and no key.
    pub warning: Option<String>,
}

/// Runs the link check of issue `number` when its claim, links or page sources changed.
///
/// No check, no parsed `form` or no evidence link gives no section. A
/// `previous` comment whose marker covers the same claim, links and page
/// sources gives its section back unchanged. A check still waiting after a failure runs no
/// model. A failure keeps the previous digest, so a later pass runs the
/// check again, and waits `retry_after_secs` from `now`.
pub(super) fn triage_issue(
    check: Option<&mut LinkCheck<'_>>,
    github: &mut RepoClient<'_>,
    form: Option<&Form>,
    previous: Option<PreviousComment<'_>>,
    cached: Option<&IssueState>,
    now: i64,
    number: u64,
) -> Triaged {
    let (Some(check), Some(form)) = (check, form) else {
        return Triaged::default();
    };
    let links = evidence_links(form);
    if links.is_empty() {
        return Triaged::default();
    }
    let claim = Claim::of_form(form);
    let digest = triage_digest(&claim, &links, check.web.is_some());
    let covered = previous.as_ref().and_then(|found| found.marker.triage);
    if covered == Some(digest)
        && let Some(section) = previous.and_then(|found| section_of_comment(found.text))
    {
        return Triaged {
            section: Some(section),
            digest: Some(digest),
            ..Triaged::default()
        };
    }
    if let Some(at) = cached
        .and_then(|state| state.triage_retry_after)
        .filter(|at| *at > now)
    {
        return Triaged {
            digest: covered,
            retry_after: Some(at),
            ..Triaged::default()
        };
    }
    let mut checkers = Checkers {
        github,
        web: check.web.as_mut(),
        llm: &mut check.llm,
    };
    let reason = match triage(&claim, &links, check.max_links, &mut checkers) {
        Ok(result) if !result.all_failed => {
            return Triaged {
                section: Some(triage_section(&result.lines)),
                digest: Some(result.digest),
                ran: true,
                ..Triaged::default()
            };
        }
        Ok(_) => "no model answered for any evidence link".to_owned(),
        Err(error) => error.to_string(),
    };
    let wait = i64::try_from(check.retry_after_secs).unwrap_or(i64::MAX);
    Triaged {
        digest: covered,
        retry_after: Some(now.saturating_add(wait)),
        warning: Some(format!(
            "issue {number}: the link check failed: {reason}. The comment shows no link check, and the check runs again in {} seconds",
            check.retry_after_secs
        )),
        ..Triaged::default()
    }
}
