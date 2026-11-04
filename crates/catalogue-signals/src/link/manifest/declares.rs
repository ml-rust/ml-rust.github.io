//! Whether one manifest declares a crate.

use catalogue_core::canonical_crate_name;
use serde::Deserialize;

/// The longest manifest read, in bytes. A longer one does not match.
pub(super) const MAX_MANIFEST: u64 = 1 << 20;

#[derive(Deserialize)]
struct Manifest {
    package: Option<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
}

/// Whether `body` is a manifest whose `[package].name` equals `crate_name`.
///
/// The names compare the way crates.io compares them: ASCII case and `-` against `_` do not
/// matter. A body past [`MAX_MANIFEST`], a body that is not UTF-8 and a body that does not parse
/// do not match. None of them is an error.
pub(super) fn declares_crate(body: &[u8], crate_name: &str) -> bool {
    if body.len() as u64 > MAX_MANIFEST {
        return false;
    }
    let Ok(text) = std::str::from_utf8(body) else {
        return false;
    };
    let Ok(manifest) = toml::from_str::<Manifest>(text) else {
        return false;
    };
    manifest.package.is_some_and(|package| {
        canonical_crate_name(&package.name) == canonical_crate_name(crate_name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_package_name_matches() {
        let body = b"[package]\nname = \"whisper-rs\"\nversion = \"1.0.0\"\n";
        assert!(declares_crate(body, "whisper-rs"));
    }

    #[test]
    fn dash_and_underscore_compare_as_equal() {
        assert!(declares_crate(
            b"[package]\nname = \"my_crate\"\n",
            "my-crate"
        ));
        assert!(declares_crate(
            b"[package]\nname = \"my-crate\"\n",
            "my_crate"
        ));
    }

    #[test]
    fn another_package_name_does_not_match() {
        assert!(!declares_crate(
            b"[package]\nname = \"other\"\n",
            "my-crate"
        ));
    }

    #[test]
    fn a_name_found_only_in_the_dependencies_does_not_match() {
        let body = b"[package]\nname = \"app\"\n\n[dependencies]\nmy-crate = \"1\"\n";
        assert!(!declares_crate(body, "my-crate"));
        let table = b"[dependencies.my-crate]\nversion = \"1\"\n";
        assert!(!declares_crate(table, "my-crate"));
    }

    #[test]
    fn a_workspace_root_does_not_match() {
        let body =
            b"[workspace]\nmembers = [\"my-crate\"]\n[workspace.package]\nname = \"my-crate\"\n";
        assert!(!declares_crate(body, "my-crate"));
    }

    #[test]
    fn a_manifest_that_does_not_parse_does_not_match() {
        for body in [
            &b"[package\nname = \"my-crate\"\n"[..],
            b"[package]\nname = 7\n",
            b"[package]\nversion = \"1\"\n",
            b"\xff\xfe[package]\nname = \"my-crate\"\n",
        ] {
            assert!(!declares_crate(body, "my-crate"));
        }
    }

    #[test]
    fn an_oversize_manifest_does_not_match() {
        let mut body = b"[package]\nname = \"my-crate\"\n".to_vec();
        let limit = usize::try_from(MAX_MANIFEST).expect("fits");
        body.resize(limit, b'\n');
        assert!(declares_crate(&body, "my-crate"));
        body.push(b'\n');
        assert!(!declares_crate(&body, "my-crate"));
    }
}
