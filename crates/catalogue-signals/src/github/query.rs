//! The search URL and its percent-encoding.

use jiff::civil::Date;

/// The repository search endpoint.
const ENDPOINT: &str = "https://api.github.com/search/repositories";

/// Repos per page, the most the API serves.
pub(super) const PER_PAGE: usize = 100;

/// The URL of page `page` for `topic`, created from `from` to `to` inclusive.
pub(super) fn search_url(topic: &str, min_stars: u32, from: Date, to: Date, page: u32) -> String {
    let q = format!(
        "language:Rust topic:{topic} stars:>={min_stars} archived:false fork:false created:{from}..{to}"
    );
    format!(
        "{ENDPOINT}?q={}&sort=stars&order=desc&per_page={PER_PAGE}&page={page}",
        percent_encode(&q)
    )
}

/// Encodes every byte outside the RFC 3986 unreserved set as `%XX`.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            for nibble in [byte >> 4, byte & 0x0f] {
                if let Some(digit) = char::from_digit(u32::from(nibble), 16) {
                    out.push(digit.to_ascii_uppercase());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    #[test]
    fn unreserved_bytes_stay_and_others_are_escaped() {
        assert_eq!(percent_encode("aZ09-._~"), "aZ09-._~");
        assert_eq!(percent_encode("a b:>=+&é"), "a%20b%3A%3E%3D%2B%26%C3%A9");
    }

    #[test]
    fn the_url_carries_the_encoded_query_and_paging() {
        let url = search_url("llm", 10, date(2008, 1, 1), date(2026, 11, 3), 2);
        assert_eq!(
            url,
            "https://api.github.com/search/repositories?q=language%3ARust%20topic%3Allm%20stars%3A%3E%3D10%20archived%3Afalse%20fork%3Afalse%20created%3A2008-01-01..2026-11-03&sort=stars&order=desc&per_page=100&page=2"
        );
    }
}
