//! The `Link` response header that pages a listing.

/// The URL of the `rel="next"` link in a `Link` header value, or `None`.
///
/// The value is a comma-separated list of `<url>; param; ...` links. A `rel`
/// parameter holds space-separated relation types, quoted or bare. Relation
/// types match without regard to case. A link without a closing `>` ends the
/// parse.
pub fn next_link(value: &str) -> Option<&str> {
    let mut rest = value;
    loop {
        let start = rest.find('<')?;
        let after = &rest[start + 1..];
        let end = after.find('>')?;
        let url = &after[..end];
        let params_and_rest = &after[end + 1..];
        let (params, next) = split_link(params_and_rest);
        if params.split(';').any(is_next_rel) {
            return Some(url);
        }
        rest = next;
    }
}

/// Splits the parameters of one link from the links after it, at the first
/// comma outside quotes.
fn split_link(text: &str) -> (&str, &str) {
    let mut quoted = false;
    for (index, byte) in text.bytes().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            b',' if !quoted => return (&text[..index], &text[index + 1..]),
            _ => {}
        }
    }
    (text, "")
}

/// Whether one `name=value` parameter is a `rel` that lists `next`.
fn is_next_rel(param: &str) -> bool {
    let Some((name, value)) = param.split_once('=') else {
        return false;
    };
    name.trim().eq_ignore_ascii_case("rel")
        && value
            .trim()
            .trim_matches('"')
            .split_ascii_whitespace()
            .any(|relation| relation.eq_ignore_ascii_case("next"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_link_of_a_github_header_is_found() {
        let value = "<https://api.github.com/repositories/1/labels?per_page=100&page=2>; rel=\"next\", \
                     <https://api.github.com/repositories/1/labels?per_page=100&page=5>; rel=\"last\"";
        assert_eq!(
            next_link(value),
            Some("https://api.github.com/repositories/1/labels?per_page=100&page=2")
        );
    }

    #[test]
    fn the_order_of_links_does_not_matter() {
        let value = "<https://x/1>; rel=\"prev\", <https://x/3>; rel=\"next\"";
        assert_eq!(next_link(value), Some("https://x/3"));
    }

    #[test]
    fn the_last_page_has_no_next_link() {
        let value = "<https://x/1>; rel=\"prev\", <https://x/1>; rel=\"first\"";
        assert_eq!(next_link(value), None);
        assert_eq!(next_link(""), None);
    }

    #[test]
    fn bare_and_multi_valued_relations_match() {
        assert_eq!(next_link("<https://x/2>; rel=next"), Some("https://x/2"));
        assert_eq!(
            next_link("<https://x/2>; title=\"a, b\"; rel=\"last NEXT\""),
            Some("https://x/2")
        );
    }

    #[test]
    fn a_quoted_comma_does_not_split_a_link() {
        let value =
            "<https://x/1>; title=\"next, please\"; rel=\"prev\", <https://x/2>; rel=\"next\"";
        assert_eq!(next_link(value), Some("https://x/2"));
    }

    #[test]
    fn a_relation_name_inside_another_word_does_not_match() {
        assert_eq!(next_link("<https://x/2>; rel=\"nextpage\""), None);
        assert_eq!(next_link("<https://x/2>; xrel=\"next\""), None);
    }

    #[test]
    fn a_broken_link_ends_the_parse() {
        assert_eq!(next_link("<https://x/2; rel=\"next\""), None);
    }
}
