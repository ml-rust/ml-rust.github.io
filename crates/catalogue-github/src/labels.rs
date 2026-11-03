//! Repo labels: list, create and update.

use catalogue_signals::http::percent_encode;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::api::{PER_PAGE, Verb};
use crate::client::RepoClient;
use crate::error::ApiError;

/// A label as the API lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Label {
    /// The label name.
    pub name: String,
    /// The color as 6 hex digits, without `#`.
    pub color: String,
    /// The description. GitHub sends `null` for none.
    #[serde(default)]
    pub description: Option<String>,
}

/// The fields of a label to create, or to write over an existing one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelFields {
    /// The label name.
    pub name: String,
    /// The color as 6 hex digits, without `#`.
    pub color: String,
    /// The description.
    pub description: String,
}

impl LabelFields {
    /// The JSON body. `name_key` is `name` for a create and `new_name` for an update.
    fn body(&self, name_key: &str) -> String {
        let mut body = Map::new();
        body.insert(name_key.to_owned(), Value::from(self.name.as_str()));
        body.insert("color".to_owned(), Value::from(self.color.as_str()));
        body.insert(
            "description".to_owned(),
            Value::from(self.description.as_str()),
        );
        Value::Object(body).to_string()
    }
}

impl RepoClient<'_> {
    /// Every label of the repo, across all pages.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when a call fails, a page is not the expected
    /// JSON, or the listing passes the page cap.
    pub fn labels(&mut self) -> Result<Vec<Label>, ApiError> {
        let url = self.repo_url(&format!("/labels?per_page={PER_PAGE}"));
        self.get_pages(&url)
    }

    /// Creates `label`.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or GitHub answers with a
    /// status other than 201. A label that exists answers 422.
    pub fn create_label(&mut self, label: &LabelFields) -> Result<(), ApiError> {
        let url = self.repo_url("/labels");
        self.call(Verb::Post, &url, Some(&label.body("name")), &[201])?;
        Ok(())
    }

    /// Writes `label` over the label now named `current`. The update can
    /// change the name, for a case fix.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or GitHub answers with a
    /// status other than 200.
    pub fn update_label(&mut self, current: &str, label: &LabelFields) -> Result<(), ApiError> {
        let url = self.repo_url(&format!("/labels/{}", percent_encode(current)));
        self.call(Verb::Patch, &url, Some(&label.body("new_name")), &[200])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listed_label_with_a_null_description_parses() {
        let label: Label = serde_json::from_str(
            r#"{"id":1,"name":"submit","color":"0e8a16","description":null,"default":false}"#,
        )
        .expect("parses");
        assert_eq!(label.description, None);
    }

    #[test]
    fn create_and_update_bodies_name_the_label_differently() {
        let fields = LabelFields {
            name: "judgment:kind".to_owned(),
            color: "1d76db".to_owned(),
            description: "Set \"kind\"".to_owned(),
        };
        let create: Value = serde_json::from_str(&fields.body("name")).expect("json");
        assert_eq!(
            create,
            serde_json::json!({"name": "judgment:kind", "color": "1d76db", "description": "Set \"kind\""})
        );
        let update: Value = serde_json::from_str(&fields.body("new_name")).expect("json");
        assert_eq!(update["new_name"], "judgment:kind");
    }
}
