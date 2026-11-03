use crate::entry::MAX_DESCRIPTION_CHARS;

/// The description text: trimmed, with every run of whitespace collapsed to one space.
///
/// Errors with the skip reason for a missing, blank or over-long description.
pub fn description_text(raw: Option<&str>) -> Result<String, String> {
    let words: Vec<&str> = raw.unwrap_or_default().split_whitespace().collect();
    let text = words.join(" ");
    if text.is_empty() {
        return Err("no description".to_owned());
    }
    if text.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "description is longer than {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_collapses_whitespace() {
        assert_eq!(
            description_text(Some("  A  tensor\n\tlibrary. ")),
            Ok("A tensor library.".to_owned())
        );
    }

    #[test]
    fn description_limit_is_300_characters() {
        let limit = "é".repeat(300);
        assert_eq!(description_text(Some(&limit)), Ok(limit.clone()));
        let over = "x".repeat(301);
        let error = description_text(Some(&over)).expect_err("over the limit");
        assert!(error.contains("longer than 300"), "{error}");
    }

    #[test]
    fn missing_or_blank_description_is_an_error() {
        assert_eq!(description_text(None), Err("no description".to_owned()));
        assert_eq!(
            description_text(Some(" \n ")),
            Err("no description".to_owned())
        );
    }
}
