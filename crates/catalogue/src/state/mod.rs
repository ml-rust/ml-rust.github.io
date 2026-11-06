//! `catalogue state`: writes the weekly state report and publishes it.

mod config;
mod publish;
mod store;

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use catalogue_core::{Date, Slug};
use catalogue_github::{GithubApp, Secret, WRITE_INTERVAL};
use catalogue_signals::http::{IntervalPacer, ReqwestTransport, Transport};
use catalogue_site::SITE_URL;
use catalogue_state::{
    Projects, ReleaseSources, changelog_text, readme_text, refresh_summaries, release_readme_text,
    write_report,
};
use catalogue_triage::{Llm, MODEL_INTERVAL, Provider, SleepBackoff};
use clap::{ArgGroup, Args};
use jiff::Timestamp;

use crate::bot::{CONFIG_VAR, config_path, load_config, permissions};
use crate::clock::today_utc;
use crate::exit::{INVALID, IO};
use config::{StateConfig, load_state_config};
pub use publish::{STATE_PATH, publish_state};
use store::SqliteStore;

/// The catalogue file of the live site.
pub const CATALOGUE_URL: &str = "https://ml-rust.github.io/data/catalogue.json";

/// The least time between two project page requests to the site.
const README_INTERVAL: Duration = Duration::from_millis(200);

/// The summaries file used when `--summaries` is not given.
pub const DEFAULT_SUMMARIES: &str = "summaries.db";

/// The config file read when `--config` is not given.
pub const DEFAULT_STATE_CONFIG: &str = "state.toml";

/// The arguments of `catalogue state`.
#[derive(Debug, Args)]
#[command(group(ArgGroup::new("target").required(true).multiple(true).args(["out", "publish", "summaries_only"])))]
pub struct StateArgs {
    /// The models and limits of the report.
    #[arg(long, value_name = "FILE", default_value = DEFAULT_STATE_CONFIG)]
    pub config: PathBuf,
    /// The catalogue file: an https address or a path.
    #[arg(long, value_name = "SOURCE", default_value = CATALOGUE_URL)]
    pub catalogue: String,
    /// Writes the report to this file.
    #[arg(long, value_name = "FILE")]
    pub out: Option<PathBuf>,
    /// Commits the report to the `signals` branch as the GitHub App of the bot config.
    #[arg(long)]
    pub publish: bool,
    /// The bot config for `--publish`. Default: $CATALOGUE_BOT_CONFIG, else bot.toml.
    #[arg(long, value_name = "FILE", requires = "publish")]
    pub bot_config: Option<PathBuf>,
    /// The SQLite file that keeps the AI summaries. A release with a summary costs no model call.
    #[arg(long, value_name = "FILE", default_value = DEFAULT_SUMMARIES)]
    pub summaries: PathBuf,
    /// Writes missing summaries, then stops with no report.
    #[arg(long)]
    pub summaries_only: bool,
    /// Limits the summaries to this project. Repeat it for more.
    #[arg(long, value_name = "SLUG", requires = "summaries_only")]
    pub only: Vec<Slug>,
}

/// Runs `catalogue state` and returns the process exit code.
///
/// Prints one line per finished part and per rejected answer on stderr. The code is 0 when the
/// report is written, 1 for a config to fix, and 2 when a read, a model or a publish step fails.
/// A failed run writes nothing, so the last report stays.
pub fn run_state(args: &StateArgs) -> ExitCode {
    let config = match load_state_config(&args.config) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(INVALID);
        }
    };
    match run(args, &config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(IO)
        }
    }
}

fn run(args: &StateArgs, config: &StateConfig) -> Result<(), String> {
    let providers = read_keys(config);
    if providers.is_empty() {
        return Err(
            "no model provider has an API key. Set a key variable the config names, then rerun"
                .to_owned(),
        );
    }
    let transport = ReqwestTransport::new().map_err(|error| error.to_string())?;
    let bytes = read_catalogue(&args.catalogue, &transport)?;
    let today = Date::from(today_utc());
    let mut projects = Projects::parse(&bytes, today).map_err(|error| error.to_string())?;
    let model_transport =
        ReqwestTransport::with_timeout(config.request_timeout).map_err(|e| e.to_string())?;
    let mut pacer = IntervalPacer::new(MODEL_INTERVAL);
    let mut backoff = SleepBackoff;
    let mut llm = Llm::new(
        &model_transport,
        &mut pacer,
        &mut backoff,
        &providers,
        config.retry,
    );
    let mut log = |line: &str| eprintln!("{line}");
    let fetch_readme = |slug: &Slug| {
        thread::sleep(README_INTERVAL);
        read_catalogue(&format!("{SITE_URL}/projects/{slug}/"), &transport)
            .ok()
            .and_then(|page| readme_text(&String::from_utf8_lossy(&page)))
    };
    let mut store = SqliteStore::open(&args.summaries, today)?;
    let fetch_sources = |slug: &Slug| ReleaseSources {
        readme: projects
            .get(slug)
            .and_then(|project| project.crate_release.as_ref())
            .and_then(|(name, version)| {
                thread::sleep(README_INTERVAL);
                let url = format!("https://static.crates.io/readmes/{name}/{name}-{version}.html");
                read_catalogue(&url, &transport).ok()
            })
            .and_then(|page| release_readme_text(&String::from_utf8_lossy(&page)))
            .or_else(|| fetch_readme(slug)),
        changelog: projects
            .get(slug)
            .and_then(|project| project.repo.strip_prefix("https://github.com/"))
            .and_then(|repo| {
                thread::sleep(README_INTERVAL);
                let repo = repo.trim_end_matches('/');
                let url = format!("https://raw.githubusercontent.com/{repo}/HEAD/CHANGELOG.md");
                read_catalogue(&url, &transport).ok()
            })
            .and_then(|text| changelog_text(&String::from_utf8_lossy(&text))),
    };
    let refreshed = refresh_summaries(
        &mut llm,
        &projects,
        &mut store,
        &args.only,
        &fetch_sources,
        &mut log,
    )?;
    log(&format!(
        "summaries: {} written, {} kept, {} failed",
        refreshed.written, refreshed.kept, refreshed.failed
    ));
    if args.summaries_only {
        return Ok(());
    }
    projects.attach_summaries(&store)?;
    let report = write_report(&mut llm, &projects, today, &fetch_readme, &mut log)
        .map_err(|error| error.to_string())?;
    let mut text = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    text.push('\n');
    if let Some(path) = &args.out {
        fs::write(path, &text)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        println!("wrote {}", path.display());
    }
    if args.publish {
        publish(args, &transport, &text)?;
    }
    Ok(())
}

/// The provider of each config table with a key in the environment.
fn read_keys(config: &StateConfig) -> Vec<Provider> {
    let mut providers = Vec::with_capacity(config.providers.len());
    for found in &config.providers {
        let Some(key) = std::env::var(&found.api_key_env)
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            eprintln!(
                "warning: provider `{}` is skipped: {} is unset or empty",
                found.name, found.api_key_env
            );
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
            Err(error) => eprintln!("warning: provider `{}` is skipped: {error}", found.name),
        }
    }
    providers
}

/// The bytes of the catalogue file at `source`: an https address or a path.
fn read_catalogue(source: &str, transport: &dyn Transport) -> Result<Vec<u8>, String> {
    if !source.starts_with("https://") {
        return fs::read(source).map_err(|error| format!("cannot read {source}: {error}"));
    }
    let mut response = transport
        .get(source, &[])
        .map_err(|error| error.to_string())?;
    if response.status != 200 {
        return Err(format!(
            "{source} answered with status {}. Rerun later",
            response.status
        ));
    }
    let mut bytes = Vec::new();
    response
        .body
        .read_to_end(&mut bytes)
        .map_err(|error| format!("the body of {source} broke off: {error}. Rerun later"))?;
    Ok(bytes)
}

fn publish(args: &StateArgs, transport: &dyn Transport, text: &str) -> Result<(), String> {
    let path = config_path(args.bot_config.as_deref(), std::env::var_os(CONFIG_VAR));
    let bot = load_config(&path).map_err(|error| error.to_string())?;
    let key =
        catalogue_github::AppKey::read(&bot.private_key_path).map_err(|error| error.to_string())?;
    let mut pacer = IntervalPacer::new(WRITE_INTERVAL);
    let clock = Timestamp::now;
    let app = GithubApp::new(transport, &mut pacer, bot.app_id.get(), key, &clock);
    let mut client = app.repo(bot.installation_id.get(), bot.repo.clone(), permissions());
    let changed = publish_state(&mut client, text).map_err(|error| error.to_string())?;
    println!(
        "{}",
        if changed {
            "published the report"
        } else {
            "the published report is the same"
        }
    );
    Ok(())
}
