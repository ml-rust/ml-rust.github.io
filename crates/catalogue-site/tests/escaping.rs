//! Data text reaches the pages escaped.

mod common;

use common::{TOPICS, catalogue, pages, thresholds};

/// Markup characters that must never reach a page raw.
const HOSTILE: &str = r#"<script>"'&"#;

#[test]
fn a_topic_name_with_markup_is_escaped() {
    let topics =
        format!("{TOPICS}\n[[topic]]\nid = \"hostile\"\nname = '''{HOSTILE}alert(1)</script>'''\n");
    let site = pages(&catalogue(&topics, Some(&thresholds(None))));
    let submit = &site["submit/index.html"];
    assert!(
        submit.contains("&#60;script&#62;&#34;&#39;&#38;alert(1)&#60;/script&#62;"),
        "{submit}"
    );
    for (path, text) in &site {
        assert!(!text.contains("<script"), "{path} holds raw markup");
    }
}
