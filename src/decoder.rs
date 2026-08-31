use std::fs::File;
use std::path::Path;
use std::time::Duration;

use rodio::{Decoder, Player, Source};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DecoderCapabilities {
    pub seek: bool,
    pub subsongs: bool,
    pub loop_metadata: bool,
    pub companion_files: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StreamProperties {
    pub duration: Option<Duration>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedBackend {
    pub id: &'static str,
    pub display_name: &'static str,
    pub capabilities: DecoderCapabilities,
}

impl SelectedBackend {
    pub fn capability_summary(self) -> String {
        let mut capabilities = Vec::new();
        if self.capabilities.seek {
            capabilities.push("seek");
        }
        if self.capabilities.subsongs {
            capabilities.push("subsongs");
        }
        if self.capabilities.loop_metadata {
            capabilities.push("loops");
        }
        if self.capabilities.companion_files {
            capabilities.push("companion files");
        }
        capabilities.join(", ")
    }
}

/// One decoding family behind Kog's shared playback contract.
///
/// Specialist backends can render through C/C++ libraries and append a custom
/// `rodio::Source` without leaking their FFI details into the playlist or UI.
pub trait DecoderBackend: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn capabilities(&self) -> DecoderCapabilities;
    fn probe(&self, path: &Path) -> Result<StreamProperties, String>;
    fn append(&self, path: &Path, player: &Player) -> Result<(), String>;

    fn accepts(&self, path: &Path) -> bool {
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            return false;
        };
        self.extensions()
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(extension))
    }
}

pub struct DecoderRegistry {
    backends: Vec<Box<dyn DecoderBackend>>,
}

impl Default for DecoderRegistry {
    fn default() -> Self {
        Self {
            backends: vec![Box::new(RodioBackend)],
        }
    }
}

impl DecoderRegistry {
    pub fn probe(&self, path: &Path) -> Result<StreamProperties, String> {
        let backend = self.select(path).ok_or_else(|| unsupported_message(path))?;
        backend.probe(path)
    }

    pub fn append(&self, path: &Path, player: &Player) -> Result<SelectedBackend, String> {
        let backend = self.select(path).ok_or_else(|| unsupported_message(path))?;
        backend.append(path, player)?;
        Ok(SelectedBackend {
            id: backend.id(),
            display_name: backend.display_name(),
            capabilities: backend.capabilities(),
        })
    }

    #[cfg(test)]
    pub fn backend_id_for(&self, path: &Path) -> Option<&'static str> {
        self.select(path).map(DecoderBackend::id)
    }

    fn select(&self, path: &Path) -> Option<&dyn DecoderBackend> {
        self.backends
            .iter()
            .map(Box::as_ref)
            .find(|backend| backend.accepts(path))
    }
}

fn unsupported_message(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown");
    format!("No installed decoder backend accepts .{extension}")
}

struct RodioBackend;

// These are the containers/codecs enabled by rodio's Symphonia-all feature.
// Selection is deliberately conservative: accepting an extension here is a
// promise that this backend will be asked to decode it, not a claim that every
// codec combination within a container is supported.
const RODIO_EXTENSIONS: &[&str] = &[
    "aac", "adts", "aif", "aifc", "aiff", "alac", "caf", "flac", "m4a", "m4b", "mka", "mkv", "mp1",
    "mp2", "mp3", "mp4", "oga", "ogg", "ogv", "opus", "wav", "wave", "webm",
];

impl DecoderBackend for RodioBackend {
    fn id(&self) -> &'static str {
        "rodio-symphonia"
    }

    fn display_name(&self) -> &'static str {
        "Symphonia"
    }

    fn extensions(&self) -> &'static [&'static str] {
        RODIO_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            ..DecoderCapabilities::default()
        }
    }

    fn probe(&self, path: &Path) -> Result<StreamProperties, String> {
        let decoder = Decoder::try_from(
            File::open(path).map_err(|error| format!("opening {}: {error}", path.display()))?,
        )
        .map_err(|error| format!("decoding {}: {error}", path.display()))?;

        Ok(StreamProperties {
            duration: decoder.total_duration(),
            sample_rate: Some(decoder.sample_rate().get()),
            channels: Some(decoder.channels().get()),
        })
    }

    fn append(&self, path: &Path, player: &Player) -> Result<(), String> {
        let decoder = Decoder::try_from(
            File::open(path).map_err(|error| format!("opening {}: {error}", path.display()))?,
        )
        .map_err(|error| format!("decoding {}: {error}", path.display()))?;
        player.append(decoder);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn write_test_wav(path: &Path) {
        let sample_rate = 8_000_u32;
        let sample_count = sample_rate / 10;
        let data_size = sample_count * 2;
        let mut file = File::create(path).expect("create wave fixture");
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36 + data_size).to_le_bytes()).unwrap();
        file.write_all(b"WAVEfmt ").unwrap();
        file.write_all(&16_u32.to_le_bytes()).unwrap();
        file.write_all(&1_u16.to_le_bytes()).unwrap();
        file.write_all(&1_u16.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        file.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
        file.write_all(&2_u16.to_le_bytes()).unwrap();
        file.write_all(&16_u16.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&data_size.to_le_bytes()).unwrap();
        for _ in 0..sample_count {
            file.write_all(&0_i16.to_le_bytes()).unwrap();
        }
    }

    #[test]
    fn registry_probes_a_real_wave_stream() {
        let path = std::env::temp_dir().join(format!("kog-decoder-{}.wav", std::process::id()));
        write_test_wav(&path);
        let registry = DecoderRegistry::default();

        let properties = registry.probe(&path).expect("probe wave fixture");

        assert_eq!(registry.backend_id_for(&path), Some("rodio-symphonia"));
        assert_eq!(properties.sample_rate, Some(8_000));
        assert_eq!(properties.channels, Some(1));
        assert_eq!(properties.duration, Some(Duration::from_millis(100)));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn registry_does_not_advertise_unimplemented_specialist_formats() {
        let registry = DecoderRegistry::default();
        assert_eq!(registry.backend_id_for(Path::new("song.sid")), None);
        assert_eq!(registry.backend_id_for(Path::new("song.mid")), None);
        assert_eq!(registry.backend_id_for(Path::new("song.spc")), None);
    }
}
