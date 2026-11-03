//! Which release assets count as downloads of the app.

/// Lowercase suffixes of assets that are not the app.
///
/// Checksum, signature, attestation and metadata files carry download traffic
/// from scripts and verifiers, so the binary count excludes them.
pub const NON_BINARY_SUFFIXES: [&str; 13] = [
    ".sha256",
    ".sha512",
    ".sha256sum",
    ".md5",
    ".sig",
    ".asc",
    ".pem",
    ".minisig",
    ".sbom",
    ".spdx.json",
    ".intoto.jsonl",
    ".txt",
    ".json",
];

/// Lowercase name prefixes of checksum list assets.
const NON_BINARY_PREFIXES: [&str; 2] = ["checksums", "sha256sums"];

/// Whether the asset `name` is a binary of the app.
///
/// The match ignores case.
pub(super) fn is_binary_asset(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !NON_BINARY_SUFFIXES
        .iter()
        .any(|suffix| lower.ends_with(suffix))
        && !NON_BINARY_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archives_and_packages_are_binaries() {
        for name in [
            "ripgrep-15.2.0-x86_64-unknown-linux-musl.tar.gz",
            "ripgrep_15.2.0-1_amd64.deb",
            "tool.exe",
            "tool-x86_64.AppImage",
        ] {
            assert!(is_binary_asset(name), "{name}");
        }
    }

    #[test]
    fn checksums_signatures_and_metadata_are_not() {
        for name in [
            "a.tar.gz.sha256",
            "a.tar.gz.SHA512",
            "a.sha256sum",
            "a.md5",
            "a.sig",
            "a.asc",
            "a.pem",
            "a.minisig",
            "a.sbom",
            "a.spdx.json",
            "a.intoto.jsonl",
            "notes.txt",
            "manifest.json",
            "checksums.txt",
            "Checksums-v1",
            "SHA256SUMS",
            "sha256sums.asc",
        ] {
            assert!(!is_binary_asset(name), "{name}");
        }
    }
}
