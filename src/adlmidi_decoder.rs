use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::adlmidi::AdlMidi;
use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};

const ADLMIDI_EXTENSIONS: &[&str] = &["hmi", "hmp", "hmq", "mus", "xmi"];
const RENDER_FRAMES: usize = 2_048;

pub struct AdlMidiBackend;

impl AdlMidiBackend {
    fn open(source: &PlaybackSource) -> Result<AdlMidi, String> {
        AdlMidi::open(&source.path, source.subsong)
    }
}

impl DecoderBackend for AdlMidiBackend {
    fn id(&self) -> &'static str {
        "adlmidi"
    }

    fn display_name(&self) -> &'static str {
        "libADLMIDI d114c31 (Nuked OPL3)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        ADLMIDI_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            subsongs: true,
            loop_metadata: true,
            ..DecoderCapabilities::default()
        }
    }

    fn accepts(&self, path: &Path) -> bool {
        let extension = path.extension().and_then(|value| value.to_str());
        if extension.is_some_and(|extension| extension.eq_ignore_ascii_case("mus")) {
            let mut signature = [0_u8; 4];
            return std::fs::File::open(path)
                .and_then(|mut file| std::io::Read::read_exact(&mut file, &mut signature))
                .is_ok_and(|_| signature == *b"MUS\x1A");
        }
        extension.is_some_and(|extension| {
            ADLMIDI_EXTENSIONS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
    }

    fn subsong_count(&self, path: &Path) -> Result<Option<u32>, String> {
        Ok(Some(AdlMidi::open(path, Some(0))?.subsong_count()))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Self::open(source)?;
        Ok(StreamProperties {
            duration: Some(decoder.duration()),
            sample_rate: Some(decoder.sample_rate()),
            channels: Some(decoder.channels()),
            title: (!decoder.title().is_empty()).then(|| decoder.title().to_owned()),
            track_number: Some(decoder.selected_subsong() + 1),
            codec: Some(format_name(&source.path).to_owned()),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(AdlMidiSource::new(Self::open(source)?));
        Ok(())
    }
}

fn format_name(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("hmi") => "HMI MIDI",
        Some("hmp" | "hmq") => "HMP MIDI",
        Some("mus") => "DMX MUS",
        Some("xmi") => "Miles XMIDI",
        _ => "MIDI",
    }
}

struct AdlMidiSource {
    decoder: AdlMidi,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl AdlMidiSource {
    fn new(decoder: AdlMidi) -> Self {
        let duration = decoder.duration();
        let channels = usize::from(decoder.channels());
        Self {
            decoder,
            duration,
            pcm: vec![0.0; RENDER_FRAMES * channels],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.decoder.channels()),
            Err(error) => {
                eprintln!("Kog libADLMIDI playback error: {error}");
                0
            }
        };
    }
}

impl Iterator for AdlMidiSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pcm_index == self.pcm_samples {
            self.fill_pcm();
        }
        let sample = *self
            .pcm
            .get(self.pcm_index)
            .filter(|_| self.pcm_index < self.pcm_samples)?;
        self.pcm_index += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Source for AdlMidiSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.decoder.channels()).expect("libADLMIDI output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.decoder.sample_rate()).expect("libADLMIDI sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.decoder.seek(position);
        self.pcm_samples = 0;
        self.pcm_index = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{DecoderRegistry, DecoderSettings};

    fn fixture_path(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "kog-adlmidi-backend-{}-{name}.mus",
            std::process::id()
        ));
        std::fs::write(&path, crate::adlmidi::test_mus_bytes())
            .expect("write generated MUS fixture");
        path
    }

    #[test]
    fn registry_content_routes_and_probes_dmx_mus() {
        let path = fixture_path("probe");
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("adlmidi"));
        let sources = registry.expand(path.clone()).expect("expand MUS");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].subsong, Some(0));
        let properties = registry.probe(&sources[0]).expect("probe MUS");
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.bits_per_sample, Some(16));
        assert_eq!(properties.track_number, Some(1));
        assert_eq!(properties.codec.as_deref(), Some("DMX MUS"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn source_renders_non_silent_pcm_seeks_and_ends() {
        let path = fixture_path("source");
        let decoder = AdlMidiBackend::open(&PlaybackSource::from_path(path.clone()))
            .expect("open generated MUS");
        let mut source = AdlMidiSource::new(decoder);
        let first = source.by_ref().take(8_820).collect::<Vec<_>>();
        assert_eq!(first.len(), 8_820);
        assert!(first.iter().any(|sample| sample.abs() > 0.000_01));
        source
            .try_seek(Duration::from_millis(50))
            .expect("seek generated MUS");
        assert!(source.take(4_410).any(|sample| sample.abs() > 0.000_01));

        let decoder = AdlMidiBackend::open(&PlaybackSource::from_path(path.clone()))
            .expect("reopen generated MUS");
        let rendered = AdlMidiSource::new(decoder).count();
        assert!(rendered > 8_820);
        assert!(rendered < 44_100 * 10);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn non_dmx_mus_keeps_the_openmpt_route() {
        let path = fixture_path("tracker");
        std::fs::write(&path, b"not a DMX MUS file").expect("write non-DMX MUS");
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("libopenmpt"));
        std::fs::remove_file(path).ok();
    }
}
