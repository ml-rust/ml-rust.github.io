//! The probe project that depends on one crate release.

use std::fs;
use std::io;
use std::path::Path;

use catalogue_core::CrateName;
use catalogue_signals::sandbox::CheckItem;

/// The longest version text a list item can hold.
const MAX_VERSION_LEN: usize = 64;

/// Checks that `item` names a valid crate and a plain version, so both are safe in a manifest.
pub(super) fn validate(item: &CheckItem) -> Result<(), String> {
    CrateName::new(item.name.as_str()).map_err(|error| error.to_string())?;
    let version = item.version.as_str();
    let allowed = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+');
    if version.is_empty()
        || version.len() > MAX_VERSION_LEN
        || !version.starts_with(|c: char| c.is_ascii_digit())
        || !version.bytes().all(allowed)
    {
        return Err(format!(
            "version `{version}` must start with a digit and hold only ASCII letters, digits, `.`, `-` and `+`"
        ));
    }
    Ok(())
}

/// The probe manifest: one exact dependency on the release with default features.
///
/// The empty `[workspace]` table keeps cargo from joining a workspace above the probe.
pub(super) fn manifest(item: &CheckItem) -> String {
    format!(
        "[package]\nname = \"sandbox-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies]\n{} = \"={}\"\n\n[workspace]\n",
        item.name, item.version
    )
}

/// Creates the probe project for `item` in the empty directory `dir`.
///
/// A directory left by an earlier probe is removed first.
pub(super) fn create(dir: &Path, item: &CheckItem) -> io::Result<()> {
    match fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::create_dir_all(dir.join("src"))?;
    fs::write(dir.join("Cargo.toml"), manifest(item))?;
    fs::write(dir.join("src").join("lib.rs"), "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, version: &str) -> CheckItem {
        CheckItem {
            entry: "alpha".to_owned(),
            name: name.to_owned(),
            version: version.to_owned(),
        }
    }

    #[test]
    fn the_manifest_pins_the_exact_release() {
        let text = manifest(&item("ndarray-ext", "0.4.1"));
        assert!(
            text.contains("\n[dependencies]\nndarray-ext = \"=0.4.1\"\n"),
            "{text}"
        );
        assert!(text.ends_with("\n[workspace]\n"), "{text}");
        let parsed: toml::Table = toml::from_str(&text).expect("valid TOML");
        assert!(parsed.contains_key("workspace"));
    }

    #[test]
    fn names_and_versions_outside_the_safe_set_are_rejected() {
        assert!(validate(&item("ndarray-ext", "0.4.1-rc.1+build.5")).is_ok());
        assert!(validate(&item("bad name", "1.0.0")).is_err());
        assert!(validate(&item("a\"]\nevil", "1.0.0")).is_err());
        assert!(validate(&item("alpha", "1.0.0\"\n[x]")).is_err());
        assert!(validate(&item("alpha", "")).is_err());
        assert!(validate(&item("alpha", "v1.0.0")).is_err());
    }
}
