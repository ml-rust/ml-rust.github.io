//! The single pass over the archive entries.

use std::io::{self, Read};
use std::path::Path;

use catalogue_core::canonical_crate_name;
use flate2::read::MultiGzDecoder;

use super::advisories::Advisories;
use super::block::parse;
use super::error::RustsecError;
use crate::archive::{is_file, name_parts};

/// Bytes read from one advisory file at most.
const MAX_FILE_BYTES: u64 = 1 << 20;

/// Reads a RustSec advisory database archive from its gzip-compressed tar stream.
///
/// The reader keeps files named `<top>/crates/<package>/<file>.md`. Other
/// entries, including the toolchain advisories under `<top>/rust/`, are
/// ignored. Each kept file starts with a fenced `toml` block. A file with no
/// such block, invalid TOML, or a bad date adds a line to
/// [`Advisories::warnings`] and the pass continues.
///
/// # Errors
///
/// Returns a [`RustsecError`] when the stream is not a readable gzip tar
/// archive, or when it holds no advisory file.
pub fn read(reader: impl Read) -> Result<Advisories, RustsecError> {
    let mut archive = tar::Archive::new(MultiGzDecoder::new(reader));
    let mut advisories = Advisories::default();
    let mut files = 0_usize;
    for entry in archive.entries().map_err(archive_error)? {
        let mut entry = entry.map_err(archive_error)?;
        if !is_file(entry.header().entry_type()) {
            continue;
        }
        let path = entry.path().map_err(archive_error)?.into_owned();
        let Some(directory) = package_directory(&path) else {
            continue;
        };
        let label = path.to_string_lossy().into_owned();
        files += 1;
        let mut bytes = Vec::new();
        entry
            .by_ref()
            .take(MAX_FILE_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|source| RustsecError::Io {
                entry: label.clone(),
                source,
            })?;
        let Ok(text) = String::from_utf8(bytes) else {
            advisories
                .warnings
                .push(format!("{label}: the file is not valid UTF-8"));
            continue;
        };
        keep(&mut advisories, &label, &directory, &text);
    }
    if files == 0 {
        return Err(RustsecError::Empty);
    }
    for list in advisories.by_crate.values_mut() {
        list.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.id.cmp(&b.id)));
    }
    Ok(advisories)
}

fn archive_error(source: io::Error) -> RustsecError {
    RustsecError::Io {
        entry: "archive".to_owned(),
        source,
    }
}

/// Parses one file and files its advisory under the canonical package name.
fn keep(advisories: &mut Advisories, label: &str, directory: &str, text: &str) {
    let parsed = match parse(text) {
        Ok(parsed) => parsed,
        Err(reason) => {
            advisories.warnings.push(format!("{label}: {reason}"));
            return;
        }
    };
    for note in parsed.notes {
        advisories.warnings.push(format!("{label}: {note}"));
    }
    let key = canonical_crate_name(&parsed.advisory.package);
    if key != canonical_crate_name(directory) {
        advisories.warnings.push(format!(
            "{label}: the directory `{directory}` and the package `{}` differ, read as `{}`",
            parsed.advisory.package, parsed.advisory.package
        ));
    }
    advisories
        .by_crate
        .entry(key)
        .or_default()
        .push(parsed.advisory);
}

/// The package directory of `<top>/crates/<package>/<file>.md`.
fn package_directory(path: &Path) -> Option<String> {
    match name_parts(path)?.as_slice() {
        [_, "crates", package, file] if file.ends_with(".md") => Some((*package).to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use jiff::civil::date;
    use tar::Header;

    use super::super::advisory::{Advisory, AdvisoryKind};
    use super::*;

    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        for (path, contents) in files {
            let mut header = Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            let name = format!("advisory-db-main/{path}");
            builder
                .append_data(&mut header, name, contents.as_bytes())
                .expect("file entry");
        }
        builder
            .into_inner()
            .expect("tar stream")
            .finish()
            .expect("gzip stream")
    }

    /// An advisory file whose `[advisory]` table carries `extra` lines.
    fn file(id: &str, package: &str, day: &str, extra: &str, versions: &str) -> String {
        format!(
            "```toml\n[advisory]\nid = \"{id}\"\npackage = \"{package}\"\ndate = \"{day}\"\n{extra}\n[versions]\n{versions}\n```\n\n# Title\n"
        )
    }

    fn read_files(files: &[(&str, &str)]) -> Advisories {
        read(archive(files).as_slice()).expect("reads")
    }

    fn ids(list: &[Advisory]) -> Vec<&str> {
        list.iter().map(|advisory| advisory.id.as_str()).collect()
    }

    #[test]
    fn reads_vulnerabilities_with_and_without_a_patch() {
        let fixed = file(
            "R1",
            "foo-bar",
            "2021-01-01",
            "url = \"https://x.test/1\"",
            "patched = [\">= 1.0.0\"]\nunaffected = [\"< 0.5.0\"]",
        );
        let open = file("R2", "foo-bar", "2022-01-01", "", "patched = []");
        let all = read_files(&[
            ("crates/foo-bar/R2.md", open.as_str()),
            ("crates/foo-bar/R1.md", fixed.as_str()),
        ]);
        let list = all.for_crate("foo-bar");
        assert_eq!(ids(list), ["R1", "R2"]);
        let first = &list[0];
        assert_eq!(first.kind, AdvisoryKind::Vulnerability);
        assert_eq!(first.patched, [">= 1.0.0"]);
        assert_eq!(first.unaffected, ["< 0.5.0"]);
        assert_eq!(first.url.as_deref(), Some("https://x.test/1"));
        assert_eq!(first.date, date(2021, 1, 1));
        assert!(list[1].patched.is_empty());
        assert!(all.warnings.is_empty(), "{:?}", all.warnings);
        let unpatched = all.unpatched_vulnerabilities("foo-bar", date(2026, 11, 3), 60);
        assert_eq!(ids_of(&unpatched), ["R2"]);
    }

    fn ids_of<'a>(list: &[&'a Advisory]) -> Vec<&'a str> {
        list.iter().map(|advisory| advisory.id.as_str()).collect()
    }

    #[test]
    fn reads_informational_kinds_and_withdrawn() {
        let gone = file(
            "R3",
            "old",
            "2021-01-01",
            "informational = \"unmaintained\"\nwithdrawn = \"2024-05-06\"",
            "patched = []",
        );
        let unsound = file(
            "R4",
            "old",
            "2021-02-01",
            "informational = \"unsound\"",
            "patched = []",
        );
        let live = file(
            "R5",
            "old",
            "2021-03-01",
            "informational = \"unmaintained\"",
            "patched = []",
        );
        let all = read_files(&[
            ("crates/old/R3.md", gone.as_str()),
            ("crates/old/R4.md", unsound.as_str()),
            ("crates/old/R5.md", live.as_str()),
        ]);
        let list = all.for_crate("old");
        assert_eq!(list[0].kind, AdvisoryKind::Unmaintained);
        assert_eq!(list[0].withdrawn, Some(date(2024, 5, 6)));
        assert_eq!(list[1].kind, AdvisoryKind::Unsound);
        let found = all.unmaintained("old").map(|a| a.id.as_str());
        assert_eq!(found, Some("R5"));
    }

    #[test]
    fn unknown_informational_is_a_notice_with_a_warning() {
        let odd = file(
            "R6",
            "odd",
            "2021-01-01",
            "informational = \"weird\"",
            "patched = []",
        );
        let all = read_files(&[("crates/odd/R6.md", odd.as_str())]);
        assert_eq!(all.for_crate("odd")[0].kind, AdvisoryKind::Notice);
        assert_eq!(all.warnings.len(), 1);
        assert!(all.warnings[0].contains("crates/odd/R6.md"));
        assert!(all.warnings[0].contains("weird"));
    }

    #[test]
    fn bad_files_warn_and_the_run_continues() {
        let good = file("R7", "good", "2021-01-01", "", "patched = []");
        let bad_date = file("R8", "good", "soon", "", "patched = []");
        let all = read_files(&[
            ("crates/good/R7.md", good.as_str()),
            ("crates/good/R8.md", bad_date.as_str()),
            ("crates/good/R9.md", "# no block\n"),
            ("crates/good/R10.md", "```toml\n[advisory\n```\n"),
        ]);
        assert_eq!(all.for_crate("good").len(), 1);
        assert_eq!(all.warnings.len(), 3, "{:?}", all.warnings);
        for name in ["R8", "R9", "R10"] {
            let path = format!("crates/good/{name}.md");
            assert!(all.warnings.iter().any(|w| w.contains(&path)), "{name}");
        }
    }

    #[test]
    fn toolchain_and_non_markdown_files_are_ignored() {
        let good = file("R11", "good", "2021-01-01", "", "patched = []");
        let all = read_files(&[
            ("crates/good/R11.md", good.as_str()),
            ("rust/std/R12.md", good.as_str()),
            ("crates/good/notes.txt", "x"),
            ("README.md", "x"),
        ]);
        assert_eq!(all.by_crate.len(), 1);
        assert!(all.warnings.is_empty());
    }

    #[test]
    fn package_field_wins_over_the_directory() {
        let moved = file("R13", "real-name", "2021-01-01", "", "patched = []");
        let all = read_files(&[("crates/other-dir/R13.md", moved.as_str())]);
        assert_eq!(all.for_crate("real_name").len(), 1);
        assert!(all.for_crate("other-dir").is_empty());
        assert_eq!(all.warnings.len(), 1);
        assert!(all.warnings[0].contains("other-dir"));
    }

    #[test]
    fn an_archive_without_advisories_is_an_error() {
        let bytes = archive(&[("README.md", "x")]);
        assert!(matches!(read(bytes.as_slice()), Err(RustsecError::Empty)));
        let garbage = read(&b"not gzip"[..]);
        assert!(matches!(garbage, Err(RustsecError::Io { .. })));
    }

    #[test]
    fn classifies_paths() {
        let cases = [
            ("a/crates/x/RUSTSEC-1.md", Some("x")),
            ("./a/crates/x/RUSTSEC-1.md", Some("x")),
            ("a/rust/x/RUSTSEC-1.md", None),
            ("a/crates/x/RUSTSEC-1.txt", None),
            ("a/crates/RUSTSEC-1.md", None),
            ("/a/crates/x/y.md", None),
        ];
        for (path, expected) in cases {
            let found = package_directory(Path::new(path));
            assert_eq!(found.as_deref(), expected, "{path}");
        }
    }
}
