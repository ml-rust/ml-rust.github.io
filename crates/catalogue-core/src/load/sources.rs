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
