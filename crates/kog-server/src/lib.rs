//! Kog's HTTP/HTTPS API: configuration, authentication, and (later) audio
//! transcoding for remote clients.
//!
//! The desktop app owns the UI; this crate owns the network surface. Keeping
//! them apart means the server can be linked, tested, and reasoned about
//! without Qt, and the UI can grow a settings pane without touching the
//! transport layer.

pub mod api;
pub mod auth;
pub mod config;
pub mod devices;
pub mod media_filter;
pub mod radio;
pub mod routes;
pub mod service;
pub mod stream;
pub mod tls;

pub use auth::{AuthMode, Credentials};
pub use stream::{StreamCache, StreamKey};
pub use service::{StreamService, StreamSource};
pub use tls::{TlsMaterial, import_pem_pair, server_config};
pub use config::{ServerConfig, TlsMode};

/// Streaming codecs the server can transcode to. The user picks the default;
/// clients may request any of them per stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamCodec {
    /// Raw ADTS AAC: every browser plays it progressively, including iOS
    /// Safari, and it streams without a container to finalize.
    #[default]
    Aac,
    /// Opus in Ogg: best quality per bit where the browser supports it.
    Opus,
    /// FLAC: lossless, for LAN listening.
    Flac,
}

impl StreamCodec {
    pub const ALL: [Self; 3] = [Self::Aac, Self::Opus, Self::Flac];

    pub const fn setting_value(self) -> &'static str {
        match self {
            Self::Aac => "aac",
            Self::Opus => "opus",
            Self::Flac => "flac",
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "aac" | "m4a" | "mp4" => Some(Self::Aac),
            "opus" | "ogg" => Some(Self::Opus),
            "flac" | "lossless" => Some(Self::Flac),
            _ => None,
        }
    }

    /// MIME type for the encoded stream.
    pub const fn content_type(self) -> &'static str {
        match self {
            Self::Aac => "audio/aac",
            Self::Opus => "audio/ogg",
            Self::Flac => "audio/flac",
        }
    }

    /// File extension for cached encodes.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Aac => "aac",
            Self::Opus => "ogg",
            Self::Flac => "flac",
        }
    }
}
