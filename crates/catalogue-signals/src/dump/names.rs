//! The dump tables the reader keeps, and the order it needs them in.

/// A table the reader keeps data from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableName {
    Categories,
    Keywords,
    Crates,
    DeletedCrates,
    Users,
    Teams,
    CratesCategories,
    CratesKeywords,
    CrateOwners,
    Versions,
    DefaultVersions,
    Dependencies,
    VersionDownloads,
}

impl TableName {
    /// Every kept table.
    pub(crate) const ALL: [Self; 13] = [
        Self::Categories,
        Self::Keywords,
        Self::Crates,
        Self::DeletedCrates,
        Self::Users,
        Self::Teams,
        Self::CratesCategories,
        Self::CratesKeywords,
        Self::CrateOwners,
        Self::Versions,
        Self::DefaultVersions,
        Self::Dependencies,
        Self::VersionDownloads,
    ];

    /// The table name, which is also the CSV file stem.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Categories => "categories",
            Self::Keywords => "keywords",
            Self::Crates => "crates",
            Self::DeletedCrates => "deleted_crates",
            Self::Users => "users",
            Self::Teams => "teams",
            Self::CratesCategories => "crates_categories",
            Self::CratesKeywords => "crates_keywords",
            Self::CrateOwners => "crate_owners",
            Self::Versions => "versions",
            Self::DefaultVersions => "default_versions",
            Self::Dependencies => "dependencies",
            Self::VersionDownloads => "version_downloads",
        }
    }

    /// The kept table whose CSV file stem is `stem`.
    pub(crate) fn from_stem(stem: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|table| table.name() == stem)
    }

    /// Tables the reader joins against while it reads this one.
    pub(crate) fn needs(self) -> &'static [Self] {
        match self {
            Self::CratesCategories => &[Self::Categories],
            Self::CratesKeywords => &[Self::Keywords],
            Self::Dependencies => &[Self::DefaultVersions],
            Self::VersionDownloads => &[Self::Versions],
            _ => &[],
        }
    }

    fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

/// The set of tables read so far.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Seen(u16);

impl Seen {
    pub(crate) fn insert(&mut self, table: TableName) {
        self.0 |= table.bit();
    }

    pub(crate) fn contains(self, table: TableName) -> bool {
        self.0 & table.bit() != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_round_trip() {
        for table in TableName::ALL {
            assert_eq!(TableName::from_stem(table.name()), Some(table));
        }
        assert_eq!(TableName::from_stem("crate_downloads"), None);
    }

    #[test]
    fn seen_tracks_each_table_apart() {
        let mut seen = Seen::default();
        seen.insert(TableName::Versions);
        assert!(seen.contains(TableName::Versions));
        for table in TableName::ALL {
            if table != TableName::Versions {
                assert!(!seen.contains(table), "{table:?}");
            }
        }
    }
}
