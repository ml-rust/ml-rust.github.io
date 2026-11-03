//! The App client and the per-repo client.

use catalogue_signals::github::RepoRef;
use catalogue_signals::http::{Answer, Pacer, Transport, percent_encode};
use jiff::Timestamp;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::api::{API_URL, Api, Verb, parse_json};
use crate::app::{AppKey, InstallationToken, Permissions, app_jwt, parse_token};
use crate::error::ApiError;
use crate::secret::Secret;

/// The clock the client reads to sign JWTs and to refresh tokens.
pub type Clock<'a> = &'a dyn Fn() -> Timestamp;

/// The `GET /app` field the client reads.
#[derive(Deserialize)]
struct AppInfo {
    slug: String,
}

/// Calls the GitHub API as a GitHub App.
///
/// Each call signs a fresh JWT with the [`Clock`]. One [`Pacer`] spaces every
/// request, so pass an `IntervalPacer` of at least
/// [`WRITE_INTERVAL`](crate::WRITE_INTERVAL).
pub struct GithubApp<'a> {
    api: Api<'a>,
    app_id: u64,
    key: AppKey,
    clock: Clock<'a>,
}

impl<'a> GithubApp<'a> {
    /// A client for App `app_id` that signs with `key`.
    pub fn new(
        transport: &'a dyn Transport,
        pacer: &'a mut dyn Pacer,
        app_id: u64,
        key: AppKey,
        clock: Clock<'a>,
    ) -> Self {
        Self {
            api: Api::new(transport, pacer),
            app_id,
            key,
            clock,
        }
    }

    /// The App slug from `GET /app`. The bot account login is `<slug>[bot]`.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the JWT cannot be signed, the call fails or
    /// the answer is not the expected JSON.
    pub fn slug(&mut self) -> Result<String, ApiError> {
        let url = format!("{API_URL}/app");
        let jwt = self.jwt()?;
        let answer = self.api.call(Verb::Get, &url, &jwt, None, &[200])?;
        let info: AppInfo = parse_json(&url, &answer.body)?;
        Ok(info.slug)
    }

    /// Mints a token for installation `installation_id`, scoped to the repo
    /// named `repository` and to `permissions`.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the JWT cannot be signed, the call fails or
    /// the answer lacks the token or its expiry.
    pub fn installation_token(
        &mut self,
        installation_id: u64,
        repository: &str,
        permissions: &Permissions,
    ) -> Result<InstallationToken, ApiError> {
        let url = format!("{API_URL}/app/installations/{installation_id}/access_tokens");
        let jwt = self.jwt()?;
        let body = permissions.token_body(repository);
        let answer = self.api.call(Verb::Post, &url, &jwt, Some(&body), &[201])?;
        parse_token(&url, &answer.body)
    }

    /// A client for `repo` through installation `installation_id`.
    ///
    /// Its token covers `repo` alone, with `permissions`. The first call mints it.
    pub fn repo(
        self,
        installation_id: u64,
        repo: RepoRef,
        permissions: Permissions,
    ) -> RepoClient<'a> {
        RepoClient {
            app: self,
            installation_id,
            repo,
            permissions,
            token: None,
        }
    }

    /// A JWT signed at the current clock time.
    fn jwt(&self) -> Result<Secret, ApiError> {
        app_jwt(&self.key, self.app_id, (self.clock)())
    }
}

/// Calls the REST API for one repo with an installation token.
///
/// The client mints the token on the first call. It mints a new one when
/// less than [`REFRESH_MARGIN`](crate::REFRESH_MARGIN) seconds remain.
pub struct RepoClient<'a> {
    app: GithubApp<'a>,
    installation_id: u64,
    repo: RepoRef,
    permissions: Permissions,
    token: Option<InstallationToken>,
}

impl<'a> RepoClient<'a> {
    /// The App client, for App-level calls such as [`GithubApp::slug`].
    pub fn app(&mut self) -> &mut GithubApp<'a> {
        &mut self.app
    }

    /// The repo the client calls.
    pub fn repo(&self) -> &RepoRef {
        &self.repo
    }

    /// The URL of `tail` under the repo, such as `/labels`.
    pub(crate) fn repo_url(&self, tail: &str) -> String {
        format!(
            "{API_URL}/repos/{}/{}{tail}",
            percent_encode(&self.repo.owner),
            percent_encode(&self.repo.name)
        )
    }

    /// Sends one call with the installation token.
    pub(crate) fn call(
        &mut self,
        verb: Verb,
        url: &str,
        body: Option<&str>,
        accepted: &[u16],
    ) -> Result<Answer, ApiError> {
        self.with_token(|api, token| api.call(verb, url, token, body, accepted))
    }

    /// Sends one GET with the installation token that asks for the media type `accept`.
    pub(crate) fn get_accept(
        &mut self,
        url: &str,
        accept: &str,
        accepted: &[u16],
    ) -> Result<Answer, ApiError> {
        self.with_token(|api, token| api.call_accept(Verb::Get, url, token, accept, None, accepted))
    }

    /// Reads every page of the listing at `url` with the installation token.
    pub(crate) fn get_pages<T: DeserializeOwned>(&mut self, url: &str) -> Result<Vec<T>, ApiError> {
        self.with_token(|api, token| api.get_pages(url, token))
    }

    /// Runs `call` with a token that has at least the refresh margin left.
    fn with_token<R>(
        &mut self,
        call: impl FnOnce(&mut Api<'a>, &Secret) -> Result<R, ApiError>,
    ) -> Result<R, ApiError> {
        let now = (self.app.clock)();
        let token = match self.token.take() {
            Some(token) if token.is_fresh(now) => token,
            _ => self.app.installation_token(
                self.installation_id,
                &self.repo.name,
                &self.permissions,
            )?,
        };
        let result = call(&mut self.app.api, token.secret());
        self.token = Some(token);
        result
    }
}
