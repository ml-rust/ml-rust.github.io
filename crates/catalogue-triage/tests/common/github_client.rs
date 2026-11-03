//! A repo client over a scripted transport, signed with the test App key.

use std::fs;
use std::path::Path;

use catalogue_github::testing::{NoWait, Reply, ScriptedTransport};
use catalogue_github::{Access, AppKey, GithubApp, Permissions, RepoClient};
use catalogue_signals::github::RepoRef;
use jiff::Timestamp;

/// The installation token URL of the test App.
pub const TOKEN_URL: &str = "https://api.github.com/app/installations/77/access_tokens";

/// A token reply that stays fresh for the whole test.
pub fn token_reply() -> Reply {
    Reply::new(
        201,
        r#"{"token":"ghs_test_token","expires_at":"2099-01-01T00:00:00Z","permissions":{}}"#,
    )
}

/// Runs `body` with a repo client for `ml-rust/ml-rust.github.io` over `transport`.
pub fn with_client<R>(
    transport: &ScriptedTransport,
    body: impl FnOnce(&mut RepoClient<'_>) -> R,
) -> R {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    fs::create_dir_all(&base).expect("target dir");
    let dir = tempfile::tempdir_in(base).expect("scratch directory");
    let key_path = dir.path().join("app.pem");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../catalogue-github/tests/fixtures/test-app-key.pem"),
        &key_path,
    )
    .expect("copy key");
    private(&key_path);
    let key = AppKey::read(&key_path).expect("key");
    let mut pacer = NoWait::default();
    let now: Timestamp = "2026-11-04T00:00:00Z".parse().expect("time");
    let clock = move || now;
    let app = GithubApp::new(transport, &mut pacer, 123_456, key, &clock);
    let mut client = app.repo(
        77,
        RepoRef::parse("ml-rust/ml-rust.github.io").expect("repo"),
        Permissions::new().with("issues", Access::Write),
    );
    body(&mut client)
}

#[cfg(unix)]
fn private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("chmod");
}

#[cfg(not(unix))]
fn private(_path: &Path) {}
