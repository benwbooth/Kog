//! Authentication for the API: a bearer token, HTTP basic credentials, or
//! nothing at all when the user has deliberately opened it up.
//!
//! Passwords are stored salted and hashed, never in the clear, because the
//! settings file lives in the user's home directory and may be synced. Token
//! and password comparisons are constant-time so they cannot be probed by
//! timing.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// How clients authenticate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    /// No authentication. Only accepted on loopback (see config validation).
    #[default]
    None,
    /// `Authorization: Bearer <token>`.
    Token,
    /// `Authorization: Basic base64(user:password)`.
    Basic,
}

/// Basic-auth credentials. The password itself is never stored: only a
/// per-installation salt and its hash.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Credentials {
    pub username: String,
    /// Hex-encoded salted hash of the password.
    pub password_hash: String,
    /// Hex-encoded random salt.
    pub salt: String,
}

impl Default for Credentials {
    fn default() -> Self {
        Self {
            username: String::new(),
            password_hash: String::new(),
            salt: generate_salt().unwrap_or_default(),
        }
    }
}

impl Credentials {
    pub fn is_usable(&self) -> bool {
        !self.username.trim().is_empty() && !self.password_hash.is_empty()
    }

    /// Hash and store a new password, refreshing the salt.
    pub fn set_password(&mut self, password: &str) -> Result<(), String> {
        let salt = generate_salt()?;
        self.password_hash = hash_password(&salt, password);
        self.salt = salt;
        Ok(())
    }

    /// Constant-time verification of a username and password pair.
    pub fn verify(&self, username: &str, password: &str) -> bool {
        if !self.is_usable() {
            return false;
        }
        let expected = hash_password(&self.salt, password);
        let user_matches = self
            .username
            .as_bytes()
            .ct_eq(username.as_bytes())
            .unwrap_u8()
            == 1;
        let password_matches = expected.as_bytes().ct_eq(self.password_hash.as_bytes()).unwrap_u8()
            == 1;
        user_matches & password_matches
    }
}

/// Salted SHA-256, iterated to slow down offline guessing of a leaked file.
/// Bumping `ROUNDS` invalidates existing hashes, so hashes carry no version:
/// change it only alongside a migration.
const ROUNDS: u32 = 120_000;

fn hash_password(salt: &str, password: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(salt.as_bytes());
    digest.update(b":");
    digest.update(password.as_bytes());
    let mut value = digest.finalize();
    for _ in 1..ROUNDS {
        let mut round = Sha256::new();
        round.update(value);
        round.update(salt.as_bytes());
        value = round.finalize();
    }
    to_hex(&value)
}

fn generate_salt() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| format!("generating a salt: {error}"))?;
    Ok(to_hex(&bytes))
}

/// A fresh API token: 32 random bytes, base64url without padding so it is
/// safe to paste into a URL or a header.
pub fn generate_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| format!("generating a token: {error}"))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn to_hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

/// Why a request was rejected. Kept separate so the transport layer can decide
/// on the status code and the `WWW-Authenticate` challenge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthError {
    /// A credential was supplied but does not match.
    Invalid,
    /// No credential was supplied.
    Missing,
}

impl AuthError {
    /// Header value for a 401 response.
    pub const fn challenge(self) -> &'static str {
        match self {
            Self::Missing => "Bearer, Basic realm=\"Kog\", charset=\"UTF-8\"",
            Self::Invalid => "Bearer error=\"invalid_token\", Basic realm=\"Kog\", charset=\"UTF-8\"",
        }
    }
}

/// Verify the `Authorization` header value against the configured mode.
/// Pure so it can be tested without a server.
pub fn authorize(header: Option<&str>, mode: AuthMode, config: &ConfigView<'_>) -> Result<(), AuthError> {
    if mode == AuthMode::None {
        return Ok(());
    }
    let Some(header) = header else {
        return Err(AuthError::Missing);
    };
    let header = header.trim();
    match mode {
        AuthMode::None => Ok(()),
        AuthMode::Token => {
            let (scheme, value) = split_scheme(header).ok_or(AuthError::Invalid)?;
            if !scheme.eq_ignore_ascii_case("bearer") {
                return Err(AuthError::Invalid);
            }
            if config.token.is_empty() {
                return Err(AuthError::Invalid);
            }
            if value.as_bytes().ct_eq(config.token.as_bytes()).unwrap_u8() == 1 {
                Ok(())
            } else {
                Err(AuthError::Invalid)
            }
        }
        AuthMode::Basic => {
            let (scheme, value) = split_scheme(header).ok_or(AuthError::Invalid)?;
            if !scheme.eq_ignore_ascii_case("basic") {
                return Err(AuthError::Invalid);
            }
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(value)
                .map_err(|_| AuthError::Invalid)?;
            let decoded = String::from_utf8(decoded).map_err(|_| AuthError::Invalid)?;
            let (username, password) = decoded.split_once(':').ok_or(AuthError::Invalid)?;
            if config.credentials.verify(username, password) {
                Ok(())
            } else {
                Err(AuthError::Invalid)
            }
        }
    }
}

/// The subset of configuration `authorize` needs, so tests can build one
/// without the full server config.
#[derive(Clone, Copy, Debug)]
pub struct ConfigView<'a> {
    pub token: &'a str,
    pub credentials: &'a Credentials,
}

fn split_scheme(header: &str) -> Option<(&str, &str)> {
    let (scheme, value) = header.split_once(char::is_whitespace)?;
    let value = value.trim();
    (!value.is_empty()).then_some((scheme, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic(username: &str, password: &str) -> String {
        let raw = format!("{username}:{password}");
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw)
        )
    }

    fn credentials(username: &str, password: &str) -> Credentials {
        let mut credentials = Credentials::default();
        credentials.username = username.to_owned();
        credentials.set_password(password).unwrap();
        credentials
    }

    #[test]
    fn token_auth_accepts_only_the_exact_token() {
        let credentials = Credentials::default();
        let config = ConfigView {
            token: "secret-token",
            credentials: &credentials,
        };
        assert_eq!(authorize(Some("Bearer secret-token"), AuthMode::Token, &config), Ok(()));
        assert_eq!(
            authorize(Some("bearer secret-token"), AuthMode::Token, &config),
            Ok(()),
            "scheme is case-insensitive"
        );
        assert_eq!(
            authorize(Some("Bearer secret-toke"), AuthMode::Token, &config),
            Err(AuthError::Invalid)
        );
        assert_eq!(authorize(None, AuthMode::Token, &config), Err(AuthError::Missing));
        assert_eq!(
            authorize(Some("Basic abc"), AuthMode::Token, &config),
            Err(AuthError::Invalid),
            "the wrong scheme must not fall through"
        );
    }

    #[test]
    fn basic_auth_verifies_the_hashed_password() {
        let credentials = credentials("ben", "hunter2");
        let config = ConfigView {
            token: "",
            credentials: &credentials,
        };
        assert_eq!(
            authorize(Some(&basic("ben", "hunter2")), AuthMode::Basic, &config),
            Ok(())
        );
        assert_eq!(
            authorize(Some(&basic("ben", "hunter3")), AuthMode::Basic, &config),
            Err(AuthError::Invalid)
        );
        assert_eq!(
            authorize(Some(&basic("someone", "hunter2")), AuthMode::Basic, &config),
            Err(AuthError::Invalid)
        );
        assert_eq!(
            authorize(Some("Basic not-base64!"), AuthMode::Basic, &config),
            Err(AuthError::Invalid)
        );
    }

    #[test]
    fn no_auth_mode_accepts_anything() {
        let credentials = Credentials::default();
        let config = ConfigView {
            token: "",
            credentials: &credentials,
        };
        assert_eq!(authorize(None, AuthMode::None, &config), Ok(()));
    }

    #[test]
    fn the_password_is_never_stored_in_the_clear() {
        let credentials = credentials("ben", "hunter2");
        let encoded = serde_json::to_string(&credentials).unwrap();
        assert!(!encoded.contains("hunter2"));
        assert!(credentials.is_usable());
    }

    #[test]
    fn setting_a_password_refreshes_the_salt() {
        let mut credentials = credentials("ben", "one");
        let first = credentials.password_hash.clone();
        let salt = credentials.salt.clone();
        credentials.set_password("one").unwrap();
        assert_ne!(credentials.salt, salt, "a new salt each time");
        assert_ne!(credentials.password_hash, first);
        assert!(credentials.verify("ben", "one"));
    }

    #[test]
    fn tokens_are_unique_and_url_safe() {
        let first = generate_token().unwrap();
        let second = generate_token().unwrap();
        assert_ne!(first, second);
        assert!(first.len() >= 40, "32 bytes of entropy");
        assert!(
            first
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "url-safe so it can go in a link: {first}"
        );
    }

    #[test]
    fn unusable_credentials_reject_everything() {
        let credentials = Credentials::default();
        assert!(!credentials.is_usable());
        assert!(!credentials.verify("", ""));
        assert!(!credentials.verify("ben", "hunter2"));
    }
}
