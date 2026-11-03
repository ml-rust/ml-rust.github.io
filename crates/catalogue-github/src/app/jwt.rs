//! The RS256 JWT that authenticates as the GitHub App.

use jiff::Timestamp;
use jsonwebtoken::{Algorithm, Header, encode};
use serde::Serialize;

use super::key::AppKey;
use crate::error::ApiError;
use crate::secret::Secret;

/// Seconds the `iat` claim lies in the past, to absorb clock drift.
const BACKDATE: i64 = 60;
/// Seconds from now to `exp`. GitHub refuses an expiry more than ten minutes ahead.
const LIFETIME: i64 = 9 * 60;

/// The claims GitHub reads from an App JWT.
#[derive(Serialize)]
struct Claims {
    iat: i64,
    exp: i64,
    iss: String,
}

/// Signs an App JWT for `app_id` at `now`.
///
/// `iat` is `now` minus 60 seconds and `exp` is `now` plus nine minutes.
/// `iss` is the App id as a string.
pub(crate) fn app_jwt(key: &AppKey, app_id: u64, now: Timestamp) -> Result<Secret, ApiError> {
    let second = now.as_second();
    let claims = Claims {
        iat: second - BACKDATE,
        exp: second + LIFETIME,
        iss: app_id.to_string(),
    };
    encode(&Header::new(Algorithm::RS256), &claims, key.encoding())
        .map(Secret::new)
        .map_err(|error| ApiError::Sign {
            reason: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
    use serde::Deserialize;

    use super::*;

    const TEST_KEY: &[u8] = include_bytes!("../../tests/fixtures/test-app-key.pem");
    const TEST_PUBLIC: &[u8] = include_bytes!("../../tests/fixtures/test-app-key.pub.pem");

    #[derive(Deserialize)]
    struct ReadClaims {
        iat: i64,
        exp: i64,
        iss: String,
    }

    #[test]
    fn the_jwt_is_rs256_backdated_and_short_lived() {
        let key = AppKey::from_pem(TEST_KEY, Path::new("k.pem")).expect("key");
        let now: Timestamp = "2026-11-04T00:00:00Z".parse().expect("time");
        let jwt = app_jwt(&key, 123_456, now).expect("signs");
        let token = jwt.expose();

        assert_eq!(decode_header(token).expect("header").alg, Algorithm::RS256);
        let public = DecodingKey::from_rsa_pem(TEST_PUBLIC).expect("public key");
        let mut validation = Validation::new(Algorithm::RS256);
        // The fixed clock lies in the past, so the expiry check is off.
        validation.validate_exp = false;
        validation.set_required_spec_claims(&["iat", "exp", "iss"]);
        validation.set_issuer(&["123456"]);
        let claims = decode::<ReadClaims>(token, &public, &validation)
            .expect("verifies")
            .claims;

        assert_eq!(claims.iss, "123456");
        assert_eq!(claims.iat, now.as_second() - 60);
        assert!(claims.iat < now.as_second());
        assert!(claims.exp - claims.iat <= 600);
        assert!(claims.exp - now.as_second() < 600);
    }

    #[test]
    fn another_key_does_not_verify_the_jwt() {
        let key = AppKey::from_pem(TEST_KEY, Path::new("k.pem")).expect("key");
        let jwt = app_jwt(&key, 1, Timestamp::UNIX_EPOCH).expect("signs");
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = false;
        let wrong = DecodingKey::from_secret(b"not the key");
        assert!(decode::<ReadClaims>(jwt.expose(), &wrong, &validation).is_err());
    }
}
