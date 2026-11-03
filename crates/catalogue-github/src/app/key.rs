//! The GitHub App private key, read from a PEM file.

use std::fmt;
use std::fs::{File, Metadata};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use jsonwebtoken::{Algorithm, EncodingKey};
use thiserror::Error;
use zeroize::Zeroizing;

/// The largest key file read, in bytes. An RSA key in PEM form is far smaller.
const MAX_KEY_BYTES: u64 = 64 << 10;

/// A key file the client refuses or cannot read.
///
/// No variant holds key bytes.
#[derive(Debug, Error)]
pub enum KeyError {
    /// The key file cannot be opened or read.
    #[error("cannot read the App private key {}: {source}. Check the path and rerun", .path.display())]
    Read {
        /// The key file.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// Group or other can read the key file.
    #[error(
        "the App private key {} has mode {mode:04o}, so group or other can read it. Run `chmod 600 {}` and rerun",
        .path.display(),
        .path.display()
    )]
    Exposed {
        /// The key file.
        path: PathBuf,
        /// The permission bits of the file.
        mode: u32,
    },
    /// The file holds no RSA private key in PEM form.
    #[error(
        "{} holds no RSA private key in PEM form. Point the config at the .pem file GitHub issued for the App",
        .path.display()
    )]
    NotRsaKey {
        /// The key file.
        path: PathBuf,
    },
}

/// The App private key, ready to sign RS256 JWTs.
///
/// `Debug` prints `[redacted]`. The PEM buffer is zeroed once the key is parsed.
pub struct AppKey {
    key: EncodingKey,
}

impl AppKey {
    /// Reads the PEM key at `path` and checks that it signs.
    ///
    /// On unix the file must grant group and other no access. GitHub issues
    /// the key in PKCS#1 form, `BEGIN RSA PRIVATE KEY`.
    ///
    /// # Errors
    ///
    /// Returns [`KeyError::Read`] when the file cannot be read,
    /// [`KeyError::Exposed`] when group or other can read it, and
    /// [`KeyError::NotRsaKey`] when it holds no RSA private key.
    pub fn read(path: &Path) -> Result<Self, KeyError> {
        let read_error = |source| KeyError::Read {
            path: path.to_path_buf(),
            source,
        };
        let file = File::open(path).map_err(read_error)?;
        let metadata = file.metadata().map_err(read_error)?;
        check_private(path, &metadata)?;
        // Full capacity up front, so the read never moves key bytes to a new buffer.
        let mut pem = Zeroizing::new(Vec::with_capacity(MAX_KEY_BYTES as usize + 1));
        file.take(MAX_KEY_BYTES + 1)
            .read_to_end(&mut pem)
            .map_err(read_error)?;
        if pem.len() as u64 > MAX_KEY_BYTES {
            return Err(KeyError::NotRsaKey {
                path: path.to_path_buf(),
            });
        }
        Self::from_pem(&pem, path)
    }

    /// Parses `pem` and signs a probe, so a bad key fails here and not at
    /// the first request. `path` names the source in errors.
    pub(crate) fn from_pem(pem: &[u8], path: &Path) -> Result<Self, KeyError> {
        let not_rsa = || KeyError::NotRsaKey {
            path: path.to_path_buf(),
        };
        let key = EncodingKey::from_rsa_pem(pem).map_err(|_| not_rsa())?;
        jsonwebtoken::crypto::sign(b"probe", &key, Algorithm::RS256).map_err(|_| not_rsa())?;
        Ok(Self { key })
    }

    /// The signing key.
    pub(crate) fn encoding(&self) -> &EncodingKey {
        &self.key
    }
}

impl fmt::Debug for AppKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Refuses a key file that group or other can access.
#[cfg(unix)]
fn check_private(path: &Path, metadata: &Metadata) -> Result<(), KeyError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = metadata.permissions().mode() & 0o7777;
    if mode & 0o077 == 0 {
        Ok(())
    } else {
        Err(KeyError::Exposed {
            path: path.to_path_buf(),
            mode,
        })
    }
}

/// Windows has no unix mode bits. Access lists guard the file there.
#[cfg(not(unix))]
fn check_private(_path: &Path, _metadata: &Metadata) -> Result<(), KeyError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test key in PKCS#1 form.
    const TEST_KEY: &[u8] = include_bytes!("../../tests/fixtures/test-app-key.pem");
    /// The public half of the test key.
    const TEST_PUBLIC: &[u8] = include_bytes!("../../tests/fixtures/test-app-key.pub.pem");

    #[test]
    fn a_pkcs1_key_parses_and_debug_hides_it() {
        let key = AppKey::from_pem(TEST_KEY, Path::new("k.pem")).expect("parses");
        assert_eq!(format!("{key:?}"), "[redacted]");
    }

    #[test]
    fn a_public_key_or_junk_is_refused_with_the_path_named() {
        for pem in [TEST_PUBLIC, b"not a key".as_slice()] {
            let error = AppKey::from_pem(pem, Path::new("k.pem")).expect_err("refused");
            assert!(matches!(error, KeyError::NotRsaKey { .. }));
            assert!(
                error
                    .to_string()
                    .starts_with("k.pem holds no RSA private key")
            );
        }
    }

    #[test]
    fn the_exposed_message_names_the_mode_and_the_fix() {
        let error = KeyError::Exposed {
            path: PathBuf::from("/etc/bot/app.pem"),
            mode: 0o644,
        };
        assert_eq!(
            error.to_string(),
            "the App private key /etc/bot/app.pem has mode 0644, so group or other can read it. Run `chmod 600 /etc/bot/app.pem` and rerun"
        );
    }
}
