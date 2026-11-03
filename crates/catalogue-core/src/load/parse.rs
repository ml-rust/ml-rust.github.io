use serde::de::DeserializeOwned;

use super::diagnostics::Diagnostics;
use super::sources::SourceFile;

/// A parsed data file with its path and file stem.
pub(crate) struct Parsed<T> {
    pub path: String,
    pub stem: String,
    pub value: T,
}

/// The file name of `path` without the `.toml` suffix.
pub(crate) fn file_stem(path: &str) -> &str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.strip_suffix(".toml").unwrap_or(name)
}

fn describe(error: &toml::de::Error, contents: &str) -> String {
    let line = error
        .span()
        .and_then(|span| contents.get(..span.start))
        .map(|before| before.matches('\n').count() + 1);
    match line {
        Some(line) => format!("line {line}: {}", error.message()),
        None => error.message().to_owned(),
    }
}

fn parse_file<T: DeserializeOwned>(file: &SourceFile, diags: &mut Diagnostics) -> Option<T> {
    match toml::from_str::<T>(&file.contents) {
        Ok(value) => Some(value),
        Err(error) => {
            diags.push(&file.path, describe(&error, &file.contents));
            None
        }
    }
}

/// Parses every file and drops the ones with errors, which are reported.
pub(crate) fn parse_all<T: DeserializeOwned>(
    files: &[SourceFile],
    diags: &mut Diagnostics,
) -> Vec<Parsed<T>> {
    files
        .iter()
        .filter_map(|file| {
            parse_file::<T>(file, diags).map(|value| Parsed {
                path: file.path.clone(),
                stem: file_stem(&file.path).to_owned(),
                value,
            })
        })
        .collect()
}

/// Parses an optional file. A missing or broken file yields the default value.
pub(crate) fn parse_optional<T: DeserializeOwned + Default>(
    file: Option<&SourceFile>,
    default_path: &str,
    diags: &mut Diagnostics,
) -> Parsed<T> {
    match file {
        Some(file) => Parsed {
            path: file.path.clone(),
            stem: file_stem(&file.path).to_owned(),
            value: parse_file(file, diags).unwrap_or_default(),
        },
        None => Parsed {
            path: default_path.to_owned(),
            stem: file_stem(default_path).to_owned(),
            value: T::default(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Maintainers;
    use crate::testing::file;

    #[test]
    fn stem_strips_directories_and_suffix() {
        assert_eq!(file_stem("data/entries/alpha.toml"), "alpha");
        assert_eq!(file_stem("alpha.toml"), "alpha");
        assert_eq!(file_stem("a\\b\\7.toml"), "7");
        assert_eq!(file_stem("notes.txt"), "notes.txt");
    }

    #[test]
    fn parse_errors_name_the_path_and_line() {
        let mut diags = Diagnostics::new();
        let files = [file("maintainers.toml", "[[maintainer]]\nlogin = = 1\n")];
        let parsed = parse_all::<Maintainers>(&files, &mut diags);
        assert!(parsed.is_empty());
        let items = diags.as_slice();
        assert_eq!(items.len(), 1);
        assert!(items.iter().all(|d| d.path == "maintainers.toml"));
        assert!(items.iter().all(|d| d.message.starts_with("line 2:")));
    }

    #[test]
    fn valid_files_parse_without_diagnostics() {
        let mut diags = Diagnostics::new();
        let files = [file("maintainers.toml", "")];
        let parsed = parse_all::<Maintainers>(&files, &mut diags);
        assert_eq!(parsed.len(), 1);
        assert!(diags.is_empty());
    }

    #[test]
    fn missing_optional_file_is_default() {
        let mut diags = Diagnostics::new();
        let parsed = parse_optional::<Maintainers>(None, "maintainers.toml", &mut diags);
        assert!(parsed.value.maintainer.is_empty());
        assert_eq!(parsed.path, "maintainers.toml");
        assert!(diags.is_empty());
    }

    #[test]
    fn broken_optional_file_is_reported_and_defaults() {
        let mut diags = Diagnostics::new();
        let source = file("maintainers.toml", "bogus = 1\n");
        let parsed = parse_optional::<Maintainers>(Some(&source), "maintainers.toml", &mut diags);
        assert!(parsed.value.maintainer.is_empty());
        assert_eq!(diags.len(), 1);
    }
}
