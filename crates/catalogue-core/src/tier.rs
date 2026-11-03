use serde::{Deserialize, Serialize};

/// How prominently the catalogue shows an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Shown first.
    Featured,
    /// Shown in the normal listing.
    Listed,
    /// Recently added.
    New,
    /// No longer maintained.
    Archived,
}

impl Tier {
    /// The lowercase name used in data files.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Featured => "featured",
            Self::Listed => "listed",
            Self::New => "new",
            Self::Archived => "archived",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Deserialize)]
    struct Wrap {
        tier: Tier,
    }

    #[test]
    fn reads_lowercase_names() {
        for (text, tier) in [
            ("featured", Tier::Featured),
            ("listed", Tier::Listed),
            ("new", Tier::New),
            ("archived", Tier::Archived),
        ] {
            let parsed: Wrap = toml::from_str(&format!("tier = \"{text}\"")).expect("tier");
            assert_eq!(parsed.tier, tier);
            assert_eq!(tier.as_str(), text);
        }
    }

    #[test]
    fn rejects_other_spellings() {
        assert!(toml::from_str::<Wrap>("tier = \"Featured\"").is_err());
        assert!(toml::from_str::<Wrap>("tier = \"gold\"").is_err());
    }
}
