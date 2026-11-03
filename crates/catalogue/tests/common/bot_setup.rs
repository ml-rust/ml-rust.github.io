//! A bot config, a private key copy and a labels file under the build directory.

use std::fs;
use std::path::{Path, PathBuf};

use catalogue::{BotConfig, load_config};
use tempfile::TempDir;

/// A scratch directory that holds `bot.toml`, `app.pem` and `labels.toml`.
pub struct BotSetup {
    /// The scratch directory, removed on drop.
    pub _dir: TempDir,
    /// The loaded config.
    pub config: BotConfig,
    /// The labels file.
    pub labels: PathBuf,
}

/// Writes the setup with `labels` as the labels file.
///
/// The config names `ml-rust/catalogue-test`, App 123456 and installation 77.
pub fn bot_setup(labels: &str) -> BotSetup {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    fs::create_dir_all(&base).expect("target dir");
    let dir = tempfile::tempdir_in(base).expect("scratch directory");
    let key = dir.path().join("app.pem");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../catalogue-github/tests/fixtures/test-app-key.pem"),
        &key,
    )
    .expect("copy key");
    private(&key);
    let config_path = dir.path().join("bot.toml");
    fs::write(
        &config_path,
        "owner = \"ml-rust\"\nrepo = \"catalogue-test\"\napp_id = 123456\n\
         installation_id = 77\nprivate_key_path = \"app.pem\"\n",
    )
    .expect("write config");
    let labels_path = dir.path().join("labels.toml");
    fs::write(&labels_path, labels).expect("write labels");
    let config = load_config(&config_path).expect("config loads");
    BotSetup {
        _dir: dir,
        config,
        labels: labels_path,
    }
}

#[cfg(unix)]
fn private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("chmod");
}

#[cfg(not(unix))]
fn private(_path: &Path) {}
