use serde::{Deserialize, Serialize};

use crate::ids::Slug;

/// Contents of `data/showcase.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Showcase {
    /// The slugs of the home page list, in display order.
    #[serde(default)]
    pub project: Vec<Slug>,
}
