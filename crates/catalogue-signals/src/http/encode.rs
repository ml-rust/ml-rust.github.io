//! Percent-encoding for URL query values and path segments.

/// Encodes every byte outside the RFC 3986 unreserved set as `%XX`.
pub(crate) fn percent_encode(text: &str) -> String {
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
    use super::*;

    #[test]
    fn unreserved_bytes_stay_and_others_are_escaped() {
        assert_eq!(percent_encode("aZ09-._~"), "aZ09-._~");
        assert_eq!(percent_encode("a b:>=+&é"), "a%20b%3A%3E%3D%2B%26%C3%A9");
    }

    #[test]
    fn path_separators_are_escaped() {
        assert_eq!(percent_encode("a/b?c#d"), "a%2Fb%3Fc%23d");
    }
}
