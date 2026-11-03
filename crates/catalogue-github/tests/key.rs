//! Reading the App private key from disk.

#[path = "common/key_files.rs"]
mod key_files;

use catalogue_github::{AppKey, KeyError};
use key_files::{key_copy, scratch};

#[test]
fn a_private_key_file_reads() {
    let dir = scratch();
    let key = AppKey::read(&key_copy(dir.path(), 0o600)).expect("reads");
    assert_eq!(format!("{key:?}"), "[redacted]");
}

#[cfg(unix)]
#[test]
fn a_key_file_that_group_or_other_can_read_is_refused() {
    for mode in [0o640, 0o604, 0o644, 0o660, 0o601] {
        let dir = scratch();
        let path = key_copy(dir.path(), mode);
        let error = AppKey::read(&path).expect_err("refused");
        assert!(
            matches!(error, KeyError::Exposed { mode: found, .. } if found == mode),
            "{mode:o}: {error}"
        );
        let message = error.to_string();
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(message.contains("chmod 600"), "{message}");
        assert!(!message.contains("PRIVATE KEY"), "{message}");
    }
}

#[test]
fn a_missing_key_file_names_the_path() {
    let dir = scratch();
    let path = dir.path().join("absent.pem");
    let error = AppKey::read(&path).expect_err("missing");
    assert!(matches!(error, KeyError::Read { .. }));
    assert!(error.to_string().contains("absent.pem"));
}

#[cfg(unix)]
#[test]
fn a_private_file_that_is_not_a_key_is_refused() {
    use std::os::unix::fs::PermissionsExt;

    let dir = scratch();
    let path = dir.path().join("app.pem");
    std::fs::write(
        &path,
        "-----BEGIN RSA PRIVATE KEY-----\nAAAA\n-----END RSA PRIVATE KEY-----\n",
    )
    .expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    let error = AppKey::read(&path).expect_err("refused");
    assert!(matches!(error, KeyError::NotRsaKey { .. }), "{error}");
}
