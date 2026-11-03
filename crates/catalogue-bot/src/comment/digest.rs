//! SHA-256 digests in lowercase hex.

use std::fmt;

use aws_lc_rs::digest::{SHA256, digest};

/// The length of a digest in bytes.
const LEN: usize = 32;

/// A SHA-256 digest. `Display` writes 64 lowercase hex digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sha256([u8; LEN]);

impl Sha256 {
    /// The digest of `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        let found = digest(&SHA256, bytes);
        let mut out = [0_u8; LEN];
        for (slot, byte) in out.iter_mut().zip(found.as_ref()) {
            *slot = *byte;
        }
        Self(out)
    }

    /// Parses 64 lowercase hex digits. Any other text gives `None`.
    pub fn parse_hex(text: &str) -> Option<Self> {
        if text.len() != LEN * 2 {
            return None;
        }
        let mut out = [0_u8; LEN];
        let (pairs, _) = text.as_bytes().as_chunks::<2>();
        for (slot, [high, low]) in out.iter_mut().zip(pairs) {
            *slot = (nibble(*high)? << 4) | nibble(*low)?;
        }
        Some(Self(out))
    }
}

/// The value of one lowercase hex digit.
fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

impl fmt::Display for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn digests_match_the_known_vectors() {
        assert_eq!(Sha256::of(b"").to_string(), EMPTY);
        assert_eq!(
            Sha256::of(b"abc").to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn hex_round_trips_and_rejects_other_text() {
        let digest = Sha256::parse_hex(EMPTY).expect("parses");
        assert_eq!(digest, Sha256::of(b""));
        for bad in [
            EMPTY.to_ascii_uppercase(),
            EMPTY[..63].to_owned(),
            format!("{EMPTY}0"),
            format!("g{}", &EMPTY[1..]),
            format!("é{}", &EMPTY[2..]),
        ] {
            assert_eq!(Sha256::parse_hex(&bad), None, "{bad}");
        }
    }
}
