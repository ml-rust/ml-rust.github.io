//! The search tags of the license and runs-on facts.

use catalogue_core::{
    Backends, CPU_ONLY, CPU_ONLY_ID, NONE_STATED, NOT_MEASURED, NOT_MEASURED_ID, RUNS_ON_SEPARATOR,
};

/// The tag of the license fact. Present only with a tier report.
pub(crate) const LICENSE_TAG: &str = "license";
/// The tag of one backend id. Present only with a tier report.
pub(crate) const RUNS_ON_TAG: &str = "runs_on";

/// The tag value of the license `text`: lowercase letters and digits, joined by `-`.
///
/// A checkbox id and a URL hold it, and an SPDX expression holds spaces and dots. `None`
/// reads as not measured.
pub(crate) fn license_value(text: Option<&str>) -> String {
    let text = text.unwrap_or(NOT_MEASURED);
    let lower = text.to_ascii_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    words.join("-")
}

/// Whether `text` states a license, as opposed to the lack of one.
pub(crate) fn is_stated_license(text: &str) -> bool {
    text != NONE_STATED && text != NOT_MEASURED
}

/// The backend ids of the `runs_on` fact value `value`.
///
/// `None`, or a value whose names the table no longer holds, reads as not measured.
pub(crate) fn runs_on_values<'a>(value: Option<&str>, table: &'a Backends) -> Vec<&'a str> {
    let Some(value) = value else {
        return vec![NOT_MEASURED_ID];
    };
    if value == CPU_ONLY {
        return vec![CPU_ONLY_ID];
    }
    let ids: Vec<&'a str> = value
        .split(RUNS_ON_SEPARATOR)
        .filter_map(|name| {
            let backend = table.backend.iter().find(|row| row.name == name)?;
            Some(backend.id.as_str())
        })
        .collect();
    if ids.is_empty() {
        vec![NOT_MEASURED_ID]
    } else {
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "[[backend]]\nid = \"cuda\"\nname = \"CUDA\"\nfeatures = [\"cuda\"]\n\n[[backend]]\nid = \"webgpu\"\nname = \"WebGPU\"\nfeatures = [\"wgpu\"]\n";

    fn table() -> Backends {
        toml_table(TABLE)
    }

    fn toml_table(text: &str) -> Backends {
        let sources = catalogue_core::DataSources {
            backends: Some(catalogue_core::SourceFile {
                path: "backends.toml".to_owned(),
                contents: text.to_owned(),
            }),
            ..catalogue_core::DataSources::default()
        };
        catalogue_core::Catalogue::load(sources)
            .unwrap_or_else(|diagnostics| panic!("invalid table: {diagnostics}"))
            .backends()
            .clone()
    }

    #[test]
    fn a_license_tag_fits_an_id_and_a_url() {
        assert_eq!(
            license_value(Some("Apache-2.0 OR MIT")),
            "apache-2-0-or-mit"
        );
        assert_eq!(license_value(Some("None stated")), "none-stated");
        assert_eq!(license_value(None), "not-measured");
    }

    #[test]
    fn only_a_real_license_counts_as_stated() {
        assert!(is_stated_license("MIT"));
        assert!(!is_stated_license(NONE_STATED));
        assert!(!is_stated_license(NOT_MEASURED));
    }

    #[test]
    fn names_map_to_ids_and_the_rest_read_as_not_measured() {
        let table = table();
        assert_eq!(
            runs_on_values(Some("CUDA, WebGPU"), &table),
            ["cuda", "webgpu"]
        );
        assert_eq!(runs_on_values(Some("CPU"), &table), ["cpu"]);
        assert_eq!(runs_on_values(None, &table), ["not-measured"]);
        assert_eq!(runs_on_values(Some("Gone"), &table), ["not-measured"]);
        assert_eq!(runs_on_values(Some("CUDA, Gone"), &table), ["cuda"]);
    }
}
