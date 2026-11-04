//! Decodes the base64 text of a GitHub contents answer.

/// The value of one base64 digit. `None` for any other byte.
fn digit(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some(u32::from(byte - b'A')),
        b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// The bytes of `text`. ASCII whitespace is skipped, because GitHub breaks the text into lines.
///
/// Returns `None` for a byte outside the alphabet, for text after the padding, and for a length
/// no encoder writes.
pub(super) fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut bits: u32 = 0;
    let mut held: u32 = 0;
    let mut digits: usize = 0;
    let mut padded = false;
    for byte in text.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == b'=' {
            padded = true;
            continue;
        }
        if padded {
            return None;
        }
        bits = (bits << 6) | digit(byte)?;
        held += 6;
        digits += 1;
        if held >= 8 {
            held -= 8;
            out.push(((bits >> held) & 0xff) as u8);
            bits &= (1 << held) - 1;
        }
    }
    (digits % 4 != 1).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_each_padding_length_and_skips_line_breaks() {
        for (text, bytes) in [
            ("", &b""[..]),
            ("Zg==", &b"f"[..]),
            ("Zm8=", &b"fo"[..]),
            ("Zm9v", &b"foo"[..]),
            ("Zm9v\nYmFy\n", &b"foobar"[..]),
            ("+/+/", &[0xfb, 0xff, 0xbf][..]),
        ] {
            assert_eq!(decode(text).as_deref(), Some(bytes), "{text}");
        }
    }

    #[test]
    fn refuses_text_no_encoder_writes() {
        for text in ["Z", "Zm9v!", "Zg==Zg", "Zm9vZ"] {
            assert_eq!(decode(text), None, "{text}");
        }
    }
}
