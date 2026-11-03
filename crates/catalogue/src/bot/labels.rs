//! `catalogue bot labels`: syncs the repo labels to the labels file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogue_bot::{
    LabelSpec, LabelsError, LabelsFile, RepoLabel, SyncPlan, parse_labels, plan_sync,
};
use catalogue_github::{
    Access, ApiError, AppKey, Clock, GithubApp, KeyError, Label, LabelFields, Permissions,
    RepoClient, WRITE_INTERVAL,
};
use catalogue_signals::http::{IntervalPacer, Pacer, ReqwestTransport, Transport};
use clap::Args;
use jiff::Timestamp;
use thiserror::Error;

use super::config::{BotConfig, CONFIG_VAR, ConfigError, config_path, load_config};
use crate::exit::{INVALID, IO};

/// The arguments of `catalogue bot labels`.
#[derive(Debug, Args)]
pub struct LabelsArgs {
    /// The bot config file. Default: $CATALOGUE_BOT_CONFIG, else bot.toml.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Print the plan and change nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// The labels file.
    #[arg(long, value_name = "FILE", default_value = ".github/labels.toml")]
    pub labels: PathBuf,
}

/// A label sync that stops before it finishes.
#[derive(Debug, Error)]
pub enum LabelsCommandError {
    /// The config file cannot be used.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// The labels file cannot be read.
    #[error("cannot read the labels file {}: {source}. Pass --labels with its path", .path.display())]
    LabelsRead {
        /// The labels file.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// The labels file breaks a rule.
    #[error("{}:\n{source}", .path.display())]
    LabelsFile {
        /// The labels file.
        path: PathBuf,
        /// Every problem in the file.
        source: LabelsError,
    },
    /// The App private key cannot be used.
    #[error(transparent)]
    Key(#[from] KeyError),
    /// A GitHub call fails.
    #[error(transparent)]
    Api(#[from] ApiError),
}

impl LabelsCommandError {
    /// The process exit code: 1 for a file or key to fix, 2 for a read or network error.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Config(
                ConfigError::Parse { .. } | ConfigError::Repo { .. } | ConfigError::Triage { .. },
            )
            | Self::LabelsFile { .. }
            | Self::Key(KeyError::Exposed { .. } | KeyError::NotRsaKey { .. }) => INVALID,
            Self::Config(ConfigError::Read { .. })
            | Self::LabelsRead { .. }
            | Self::Key(KeyError::Read { .. })
            | Self::Api(_) => IO,
        }
    }
}

/// What a label sync planned and whether it applied the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelsReport {
    /// The planned changes and the unknown labels.
    pub plan: SyncPlan,
    /// Whether the changes went to GitHub.
    pub applied: bool,
}

impl LabelsReport {
    /// The plan lines, then a summary line.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for line in self.plan.lines() {
            out.push_str(&line);
            out.push('\n');
        }
        let changes = self.plan.create.len() + self.plan.update.len();
        let summary = if self.plan.is_empty() {
            "labels match the labels file".to_owned()
        } else if self.applied {
            format!(
                "applied {} create(s) and {} update(s)",
                self.plan.create.len(),
                self.plan.update.len()
            )
        } else {
            format!("dry run: {changes} change(s) not applied")
        };
        out.push_str(&summary);
        out.push('\n');
        out
    }
}

/// The permissions the bot token asks for. Labels, comments and issue events
/// sit under `issues`. The data files, the tier snapshots and the bot branch
/// commits sit under `contents`. The data PRs sit under `pull_requests`.
pub(crate) fn permissions() -> Permissions {
    Permissions::new()
        .with("issues", Access::Write)
        .with("contents", Access::Write)
        .with("pull_requests", Access::Write)
}

/// Syncs the labels of the configured repo to the file at `labels_path`.
///
/// The labels file and the key are read before any request. The sync
/// creates missing labels and updates differing ones. It never deletes a
/// label. With `dry_run` it only plans. The `pacer` spaces every request.
///
/// # Errors
///
/// Returns a [`LabelsCommandError`] when the labels file or the key cannot be
/// used, or a GitHub call fails. Changes applied before a failed call stay.
pub fn sync_labels(
    transport: &dyn Transport,
    pacer: &mut dyn Pacer,
    clock: Clock<'_>,
    config: &BotConfig,
    labels_path: &Path,
    dry_run: bool,
) -> Result<LabelsReport, LabelsCommandError> {
    let file = read_labels_file(labels_path)?;
    let key = AppKey::read(&config.private_key_path)?;
    let app = GithubApp::new(transport, pacer, config.app_id.get(), key, clock);
    let mut client = app.repo(
        config.installation_id.get(),
        config.repo.clone(),
        permissions(),
    );
    sync_with(&mut client, &file, dry_run).map_err(LabelsCommandError::from)
}

/// Reads and checks the labels file at `path`.
pub(crate) fn read_labels_file(path: &Path) -> Result<LabelsFile, LabelsCommandError> {
    let text = fs::read_to_string(path).map_err(|source| LabelsCommandError::LabelsRead {
        path: path.to_path_buf(),
        source,
    })?;
    parse_labels(&text).map_err(|source| LabelsCommandError::LabelsFile {
        path: path.to_path_buf(),
        source,
    })
}

/// Syncs the labels of the repo of `client` to `file`. With `dry_run` it only plans.
///
/// Changes applied before a failed call stay.
pub(crate) fn sync_with(
    client: &mut RepoClient<'_>,
    file: &LabelsFile,
    dry_run: bool,
) -> Result<LabelsReport, ApiError> {
    let existing: Vec<RepoLabel> = client.labels()?.into_iter().map(repo_label).collect();
    let plan = plan_sync(file, &existing);
    if !dry_run {
        for label in &plan.create {
            client.create_label(&fields(label))?;
        }
        for update in &plan.update {
            client.update_label(&update.current.name, &fields(&update.wanted))?;
        }
    }
    Ok(LabelsReport {
        plan,
        applied: !dry_run,
    })
}

/// Runs `catalogue bot labels` and returns the process exit code.
///
/// Prints the plan and a summary on stdout. The code is 0 on success, 1 for
/// a config, labels file or key to fix, and 2 for a read or network error.
pub(crate) fn run_labels(args: &LabelsArgs) -> ExitCode {
    let path = config_path(args.config.as_deref(), std::env::var_os(CONFIG_VAR));
    let result = load_config(&path)
        .map_err(LabelsCommandError::from)
        .and_then(|config| {
            let transport = ReqwestTransport::new().map_err(ApiError::from)?;
            let mut pacer = IntervalPacer::new(WRITE_INTERVAL);
            let clock = Timestamp::now;
            sync_labels(
                &transport,
                &mut pacer,
                &clock,
                &config,
                &args.labels,
                args.dry_run,
            )
        });
    match result {
        Ok(report) => {
            print!("{}", report.render());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

/// A listed label in the plan's form. No description reads as empty.
fn repo_label(label: Label) -> RepoLabel {
    RepoLabel {
        name: label.name,
        color: label.color,
        description: label.description.unwrap_or_default(),
    }
}

/// The API fields of a label from the file.
fn fields(label: &LabelSpec) -> LabelFields {
    LabelFields {
        name: label.name.clone(),
        color: label.color.clone(),
        description: label.description.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_says_what_happened() {
        let file =
            parse_labels("[[label]]\nname = \"a\"\ncolor = \"000000\"\ndescription = \"\"\n")
                .expect("parses");
        let plan = plan_sync(&file, &[]);
        let dry = LabelsReport {
            plan: plan.clone(),
            applied: false,
        };
        assert!(
            dry.render()
                .ends_with("\ndry run: 1 change(s) not applied\n")
        );
        let applied = LabelsReport {
            plan,
            applied: true,
        };
        assert!(
            applied
                .render()
                .ends_with("\napplied 1 create(s) and 0 update(s)\n")
        );
        let same = LabelsReport {
            plan: SyncPlan::default(),
            applied: true,
        };
        assert_eq!(same.render(), "labels match the labels file\n");
    }

    #[test]
    fn exit_codes_split_fixes_from_read_errors() {
        let path = PathBuf::from("k.pem");
        assert_eq!(
            LabelsCommandError::Key(KeyError::Exposed {
                path: path.clone(),
                mode: 0o644
            })
            .exit_code(),
            INVALID
        );
        assert_eq!(
            LabelsCommandError::Key(KeyError::Read {
                path,
                source: io::Error::other("x")
            })
            .exit_code(),
            IO
        );
    }
}
