//! A credential string that never prints.

use std::fmt;

use zeroize::Zeroizing;

/// A credential: an App JWT or an installation token.
///
/// `Debug` prints `[redacted]`. The type has no `Display`. The memory is
/// zeroed when the value drops.
#[derive(Clone)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    /// Wraps `value`. The string moves in without a copy.
    pub(crate) fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    /// The credential text, for an `Authorization` header only.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_value() {
        let secret = Secret::new("ghs_secret".to_owned());
        assert_eq!(format!("{secret:?}"), "[redacted]");
        assert_eq!(secret.expose(), "ghs_secret");
    }
}
