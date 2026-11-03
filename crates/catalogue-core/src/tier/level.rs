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
    /// Every tier, from the most to the least prominent.
    pub const ALL: [Self; 4] = [Self::Featured, Self::Listed, Self::New, Self::Archived];

    /// The tier whose [`Tier::as_str`] name is `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tier| tier.as_str() == name)
    }

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
            assert_eq!(Tier::from_name(text), Some(tier));
        }
        assert_eq!(Tier::from_name("Featured"), None);
    }

    #[derive(Serialize)]
    struct Out {
        tier: Tier,
    }

    #[test]
    fn writes_the_tier_names() {
        for tier in Tier::ALL {
            let text = toml::to_string(&Out { tier }).expect("serializes");
            assert_eq!(text, format!("tier = \"{}\"\n", tier.as_str()));
        }
    }

    #[test]
    fn rejects_other_spellings() {
        assert!(toml::from_str::<Wrap>("tier = \"Featured\"").is_err());
        assert!(toml::from_str::<Wrap>("tier = \"gold\"").is_err());
    }
}
