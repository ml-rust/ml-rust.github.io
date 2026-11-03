use std::ffi::OsStr;
use std::path::Path;

/// The text of one data file with the path it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// Path used in diagnostics. The final component is the file name.
    pub path: String,
    /// Full file text.
    pub contents: String,
}

/// Raw text of every data file.
///
/// A missing optional file reads as an empty list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataSources {
    /// Files from `data/entries/`.
    pub entries: Vec<SourceFile>,
    /// Files from `data/judgments/`.
    pub judgments: Vec<SourceFile>,
    /// `data/maintainers.toml`.
    pub maintainers: Option<SourceFile>,
    /// `data/topics.toml`.
    pub topics: Option<SourceFile>,
    /// `data/synonyms.toml`.
    pub synonyms: Option<SourceFile>,
    /// `data/discovery.toml`.
    pub discovery: Option<SourceFile>,
    /// `data/thresholds.toml`.
    pub thresholds: Option<SourceFile>,
    /// `data/deprecated.toml`.
    pub deprecated: Option<SourceFile>,
}

/// Where one file of the data directory goes in [`DataSources`].
///
/// Every reader of a data directory maps paths through this type, so a disk
/// directory and an archive load the same files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataPlace {
    /// A file in `entries/`.
    Entries,
    /// A file in `judgments/`.
    Judgments,
    /// `maintainers.toml`.
    Maintainers,
    /// `topics.toml`.
    Topics,
    /// `synonyms.toml`.
    Synonyms,
    /// `discovery.toml`.
    Discovery,
    /// `thresholds.toml`.
    Thresholds,
    /// `deprecated.toml`.
    Deprecated,
}

/// The extension of every data file.
const TOML_EXTENSION: &str = "toml";

impl DataPlace {
    /// The places that hold one file per record, in load order.
    pub const LISTS: [Self; 2] = [Self::Entries, Self::Judgments];

    /// The places that hold one file, in load order.
    pub const FILES: [Self; 6] = [
        Self::Maintainers,
        Self::Topics,
        Self::Synonyms,
        Self::Discovery,
        Self::Thresholds,
        Self::Deprecated,
    ];

    /// The name inside the data directory: a directory name for a list place,
    /// a file name otherwise.
    pub fn name(self) -> &'static str {
        match self {
            Self::Entries => "entries",
            Self::Judgments => "judgments",
            Self::Maintainers => "maintainers.toml",
            Self::Topics => "topics.toml",
            Self::Synonyms => "synonyms.toml",
            Self::Discovery => "discovery.toml",
            Self::Thresholds => "thresholds.toml",
            Self::Deprecated => "deprecated.toml",
        }
    }

    /// Whether `file_name` names a record file in a list place: its
    /// extension is `toml`.
    pub fn is_record_name(file_name: &str) -> bool {
        Path::new(file_name).extension().and_then(OsStr::to_str) == Some(TOML_EXTENSION)
    }

    /// The place of the file at `parts`, the path inside the data directory
    /// split at `/`. `None` for any other path.
    pub fn of(parts: &[&str]) -> Option<Self> {
        match parts {
            [file] => Self::FILES.into_iter().find(|place| place.name() == *file),
            [dir, file] if Self::is_record_name(file) => {
                Self::LISTS.into_iter().find(|place| place.name() == *dir)
            }
            _ => None,
        }
    }
}

impl DataSources {
    /// Puts `file` at `place`. A list place appends. A one-file place replaces.
    pub fn put(&mut self, place: DataPlace, file: SourceFile) {
        match place {
            DataPlace::Entries => self.entries.push(file),
            DataPlace::Judgments => self.judgments.push(file),
            DataPlace::Maintainers => self.maintainers = Some(file),
            DataPlace::Topics => self.topics = Some(file),
            DataPlace::Synonyms => self.synonyms = Some(file),
            DataPlace::Discovery => self.discovery = Some(file),
            DataPlace::Thresholds => self.thresholds = Some(file),
            DataPlace::Deprecated => self.deprecated = Some(file),
        }
    }

    /// Sorts the files of each list place by path.
    pub fn sort(&mut self) {
        self.entries.sort_by(|a, b| a.path.cmp(&b.path));
        self.judgments.sort_by(|a, b| a.path.cmp(&b.path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            contents: String::new(),
        }
    }

    #[test]
    fn paths_map_to_their_place() {
        assert_eq!(
            DataPlace::of(&["entries", "a.toml"]),
            Some(DataPlace::Entries)
        );
        assert_eq!(
            DataPlace::of(&["judgments", "7.toml"]),
            Some(DataPlace::Judgments)
        );
        assert_eq!(DataPlace::of(&["topics.toml"]), Some(DataPlace::Topics));
        for parts in [
            &["entries", "a.txt"][..],
            &["entries", ".toml"],
            &["entries", "x", "a.toml"],
            &["other", "a.toml"],
            &["entries"],
            &["LICENSE"],
            &["topics.toml", "a.toml"],
            &[],
        ] {
            assert_eq!(DataPlace::of(parts), None, "{parts:?}");
        }
    }

    #[test]
    fn every_place_has_a_distinct_name() {
        let mut names: Vec<&str> = DataPlace::LISTS
            .into_iter()
            .chain(DataPlace::FILES)
            .map(DataPlace::name)
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 8);
    }

    #[test]
    fn put_appends_lists_and_replaces_files() {
        let mut sources = DataSources::default();
        sources.put(DataPlace::Entries, file("b"));
        sources.put(DataPlace::Entries, file("a"));
        sources.put(DataPlace::Topics, file("t1"));
        sources.put(DataPlace::Topics, file("t2"));
        sources.sort();
        let paths: Vec<&str> = sources.entries.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["a", "b"]);
        assert_eq!(sources.topics, Some(file("t2")));
    }
}
