//! Server configuration: what to bind, how to secure it, and how to log in.
//!
//! Stored as a small JSON file beside Kog's other settings so the desktop
//! Preferences pane and the server agree on one source of truth. Defaults are
//! deliberately closed: the server is off, and when enabled it listens on
//! loopback with token auth until the user says otherwise.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{AuthMode, Credentials, StreamCodec};

/// How the API is protected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TlsMode {
    /// Plain HTTP. Acceptable on loopback; warned about elsewhere.
    #[default]
    Off,
    /// Generate and use a self-signed certificate.
    SelfSigned,
    /// Use a certificate and key the user supplied.
    Pem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TlsSettings {
    pub mode: TlsMode,
    /// PEM certificate chain for [`TlsMode::Pem`].
    pub certificate_path: PathBuf,
    /// PEM private key for [`TlsMode::Pem`].
    pub private_key_path: PathBuf,
}

impl Default for TlsSettings {
    fn default() -> Self {
        Self {
            mode: TlsMode::Off,
            certificate_path: PathBuf::new(),
            private_key_path: PathBuf::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Master switch. Off by default: nothing listens until the user opts in.
    pub enabled: bool,
    /// Address to bind. Loopback by default.
    pub address: IpAddr,
    pub port: u16,
    pub tls: TlsSettings,
    pub auth: AuthMode,
    /// Bearer token for [`AuthMode::Token`].
    pub token: String,
    /// Username and password for [`AuthMode::Basic`].
    pub credentials: Credentials,
    /// Codec clients get when they do not ask for one.
    pub default_codec: StreamCodec,
    /// Cap on the encoded-stream cache, in bytes.
    pub cache_bytes: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8420,
            tls: TlsSettings::default(),
            auth: AuthMode::default(),
            token: String::new(),
            credentials: Credentials::default(),
            default_codec: StreamCodec::default(),
            cache_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

impl ServerConfig {
    pub fn socket_address(&self) -> SocketAddr {
        SocketAddr::new(self.address, self.port)
    }

    /// Loopback-only binds are safe by default; anything else needs the user
    /// to have chosen auth or TLS deliberately.
    pub fn is_loopback(&self) -> bool {
        self.address.is_loopback()
    }

    /// Problems that make the configuration unusable, in the order the user
    /// should fix them. Empty means the server can start.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.port == 0 {
            problems.push("Choose a port between 1 and 65535".to_owned());
        }
        match self.auth {
            AuthMode::Token if self.token.trim().is_empty() => {
                problems.push("Generate or enter an API token, or disable authentication".to_owned());
            }
            AuthMode::Basic if !self.credentials.is_usable() => {
                problems.push("Enter a username and password, or disable authentication".to_owned());
            }
            _ => {}
        }
        if !self.is_loopback() && self.auth == AuthMode::None {
            problems.push(
                "Serving other machines without authentication is not allowed; add a token or bind loopback"
                    .to_owned(),
            );
        }
        if self.tls.mode == TlsMode::Pem
            && (self.tls.certificate_path.as_os_str().is_empty()
                || self.tls.private_key_path.as_os_str().is_empty())
        {
            problems.push("Choose both a certificate and a private key".to_owned());
        }
        problems
    }

    pub fn validate(&self) -> Result<(), String> {
        let problems = self.problems();
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("; "))
        }
    }

    /// Fill in a token if the user picked token auth without one, so enabling
    /// the server never leaves it open by accident. A failed RNG leaves the
    /// token empty, which validation then refuses: never fall back to a
    /// predictable token.
    pub fn ensure_credentials(&mut self) {
        if self.auth == AuthMode::Token && self.token.trim().is_empty() {
            if let Ok(token) = crate::auth::generate_token() {
                self.token = token;
            }
        }
        if self.auth == AuthMode::Basic && !self.credentials.is_usable() {
            self.credentials = Credentials::default();
        }
    }
}

fn settings_path(file_name: &str) -> Option<PathBuf> {
    kog_audio::settings::setting_path(file_name)
}

const CONFIG_FILE: &str = "server.json";

/// Load the saved configuration, falling back to defaults.
pub fn load_config() -> ServerConfig {
    settings_path(CONFIG_FILE)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_config(config: &ServerConfig) -> Result<(), String> {
    let path = settings_path(CONFIG_FILE)
        .ok_or_else(|| "Kog's settings directory is unavailable".to_owned())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("creating {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(config)
        .map_err(|error| format!("encoding the server configuration: {error}"))?;
    write_private(&path, text.as_bytes())
}

/// Write with owner-only permissions: the file can hold a token or password.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("writing {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("writing {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_closed_and_loopback_only() {
        let config = ServerConfig::default();
        assert!(!config.enabled, "the server must be off until asked for");
        assert!(config.is_loopback());
        assert!(config.problems().is_empty());
    }

    #[test]
    fn non_loopback_without_auth_is_rejected() {
        let mut config = ServerConfig {
            address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            auth: AuthMode::None,
            ..ServerConfig::default()
        };
        assert!(config.validate().is_err());
        config.auth = AuthMode::Token;
        config.ensure_credentials();
        assert!(config.validate().is_ok(), "a generated token should satisfy it");
        assert!(!config.token.is_empty());
    }

    #[test]
    fn token_auth_without_a_token_is_incomplete() {
        let config = ServerConfig {
            auth: AuthMode::Token,
            ..ServerConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn pem_tls_needs_both_files() {
        let config = ServerConfig {
            tls: TlsSettings {
                mode: TlsMode::Pem,
                certificate_path: PathBuf::from("/tmp/cert.pem"),
                private_key_path: PathBuf::new(),
            },
            ..ServerConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_round_trips_through_json() {
        let config = ServerConfig {
            enabled: true,
            port: 9000,
            auth: AuthMode::Token,
            token: "abc123".to_owned(),
            default_codec: StreamCodec::Opus,
            ..ServerConfig::default()
        };
        let text = serde_json::to_string(&config).unwrap();
        let restored: ServerConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(restored.port, 9000);
        assert_eq!(restored.token, "abc123");
        assert_eq!(restored.default_codec, StreamCodec::Opus);
    }

    #[test]
    fn every_codec_round_trips_through_its_setting_value() {
        for codec in StreamCodec::ALL {
            assert_eq!(StreamCodec::from_setting(codec.setting_value()), Some(codec));
            assert!(!codec.content_type().is_empty());
            assert!(!codec.extension().is_empty());
        }
        assert_eq!(StreamCodec::from_setting("nonsense"), None);
    }
}
