use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::gme::{GameMusicEmu, GmePlaybackPlan, GmeTrackInfo};

const GME_EXTENSIONS: &[&str] = &["ay", "gbs", "hes", "kss", "nsf", "nsfe", "sap", "spc"];
const GME_MULTITRACK_EXTENSIONS: &[&str] = &["ay", "gbs", "hes", "kss", "nsf", "nsfe", "sap"];
const GME_DEFAULT_SAMPLE_RATE: u32 = 44_100;
const GME_SPC_SAMPLE_RATE: u32 = 32_000;
const GME_CHANNELS: u16 = 2;
const GME_RENDER_FRAMES: usize = 1_024;
const GME_DEFAULT_LENGTH: Duration = Duration::from_secs(150);
const GME_DEFAULT_FADE: Duration = Duration::from_secs(8);
const GME_DEFAULT_LOOP_COUNT: u32 = 2;

pub struct GmeBackend;

impl GmeBackend {
    fn sample_rate(path: &Path) -> u32 {
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("spc"))
        {
            GME_SPC_SAMPLE_RATE
        } else {
            GME_DEFAULT_SAMPLE_RATE
        }
    }

    fn track_and_plan(
        emu: &GameMusicEmu,
        source: &PlaybackSource,
    ) -> Result<(u32, GmeTrackInfo, GmePlaybackPlan), String> {
        let track = source.subsong.unwrap_or(0);
        let count = emu.track_count()?;
        if track >= count {
            return Err(format!(
                "{} requests GME subsong {}, but the file contains {count}",
                source.path.display(),
                track + 1
            ));
        }
        let info = emu.track_info(track)?;
        let plan = info.playback_plan(GME_DEFAULT_LENGTH, GME_DEFAULT_FADE, GME_DEFAULT_LOOP_COUNT);
        Ok((track, info, plan))
    }
}

impl DecoderBackend for GmeBackend {
    fn id(&self) -> &'static str {
        "game-music-emu"
    }

    fn display_name(&self) -> &'static str {
        "Game Music Emu 0.6.5"
    }

    fn extensions(&self) -> &'static [&'static str] {
        GME_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            subsongs: true,
            loop_metadata: true,
            companion_files: true,
        }
    }

    fn subsong_count(&self, path: &Path) -> Result<Option<u32>, String> {
        let multitrack = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| {
                GME_MULTITRACK_EXTENSIONS
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(extension))
            });
        if !multitrack {
            return Ok(None);
        }
        let emu = GameMusicEmu::open(path, -1)?;
        Ok(Some(emu.track_count()?))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let emu = GameMusicEmu::open(&source.path, -1)?;
        let (track, info, plan) = Self::track_and_plan(&emu, source)?;
        Ok(StreamProperties {
            duration: Some(Duration::from_millis(plan.total_length_ms)),
            sample_rate: Some(Self::sample_rate(&source.path)),
            channels: Some(GME_CHANNELS),
            title: nonempty(info.song),
            artist: nonempty(info.author),
            album: nonempty(info.game),
            genre: nonempty(info.system),
            track_number: Some(track + 1),
            warning: emu.warning.clone(),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        let sample_rate = Self::sample_rate(&source.path);
        let mut emu = GameMusicEmu::open(
            &source.path,
            i32::try_from(sample_rate).expect("GME sample rate fits i32"),
        )?;
        let (track, _, plan) = Self::track_and_plan(&emu, source)?;
        emu.start_track(track, plan)?;
        player.append(GmeSource::new(emu, sample_rate, plan)?);
        Ok(())
    }
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

struct GmeSource {
    emu: GameMusicEmu,
    sample_rate: u32,
    duration: Duration,
    nominal_total_frames: u64,
    total_frames: u64,
    frames_rendered: u64,
    samples_emitted: u64,
    pcm: Vec<i16>,
    interleaved: Vec<f32>,
    interleaved_index: usize,
}

impl GmeSource {
    fn new(emu: GameMusicEmu, sample_rate: u32, plan: GmePlaybackPlan) -> Result<Self, String> {
        let total_frames =
            (u128::from(plan.total_length_ms) * u128::from(sample_rate)).div_ceil(1_000);
        let total_frames = u64::try_from(total_frames)
            .map_err(|_| "Game Music Emu duration exceeds Kog's limit".to_owned())?;
        Ok(Self {
            emu,
            sample_rate,
            duration: Duration::from_millis(plan.total_length_ms),
            nominal_total_frames: total_frames,
            total_frames,
            frames_rendered: 0,
            samples_emitted: 0,
            pcm: vec![0; GME_RENDER_FRAMES * usize::from(GME_CHANNELS)],
            interleaved: Vec::with_capacity(GME_RENDER_FRAMES * usize::from(GME_CHANNELS)),
            interleaved_index: 0,
        })
    }

    fn fill_interleaved(&mut self) {
        self.interleaved.clear();
        self.interleaved_index = 0;
        if self.emu.track_ended() {
            self.total_frames = self.frames_rendered;
            return;
        }
        let frames = usize::try_from(self.total_frames.saturating_sub(self.frames_rendered))
            .unwrap_or(usize::MAX)
            .min(GME_RENDER_FRAMES);
        if frames == 0 {
            return;
        }
        if let Err(error) = self.emu.render(&mut self.pcm[..frames * 2]) {
            eprintln!("Kog Game Music Emu playback error: {error}");
            self.total_frames = self.frames_rendered;
            return;
        }
        self.interleaved.extend(
            self.pcm[..frames * 2]
                .iter()
                .map(|sample| f32::from(*sample) * (1.0 / 32_768.0)),
        );
        self.frames_rendered += frames as u64;
    }

    fn seek_to(&mut self, position: Duration) -> Result<(), String> {
        let target = position.min(self.duration);
        self.emu.seek(target)?;
        let target_frames = target
            .as_millis()
            .saturating_mul(u128::from(self.sample_rate))
            / 1_000;
        let target_frames = u64::try_from(target_frames)
            .map_err(|_| "Game Music Emu seek position exceeds Kog's limit".to_owned())?;
        self.total_frames = self.nominal_total_frames;
        self.frames_rendered = target_frames;
        self.samples_emitted = target_frames * u64::from(GME_CHANNELS);
        self.interleaved.clear();
        self.interleaved_index = 0;
        Ok(())
    }
}

impl Iterator for GmeSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let total_samples = self.total_frames * u64::from(GME_CHANNELS);
        if self.samples_emitted >= total_samples {
            return None;
        }
        if self.interleaved_index == self.interleaved.len() {
            self.fill_interleaved();
        }
        let sample = *self.interleaved.get(self.interleaved_index)?;
        self.interleaved_index += 1;
        self.samples_emitted += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self
            .total_frames
            .saturating_mul(u64::from(GME_CHANNELS))
            .saturating_sub(self.samples_emitted);
        let remaining = usize::try_from(remaining).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl Source for GmeSource {
    fn current_span_len(&self) -> Option<usize> {
        self.size_hint().1
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(GME_CHANNELS).expect("GME output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("GME sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.seek_to(position)
            .map_err(|error| SeekError::Other(Arc::new(std::io::Error::other(error))))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::decoder::{DecoderRegistry, DecoderSettings};

    fn test_nsf_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("native/game-music-emu/test.nsf")
    }

    #[test]
    fn registry_expands_and_probes_the_official_nsf_fixture() {
        let registry = DecoderRegistry::new(DecoderSettings::default());
        let sources = registry.expand(test_nsf_path()).expect("expand NSF");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].subsong, Some(0));
        assert_eq!(
            registry.backend_id_for(&sources[0].path),
            Some("game-music-emu")
        );
        let properties = registry.probe(&sources[0]).expect("probe NSF subsong");
        assert_eq!(properties.title.as_deref(), Some("BGM C"));
        assert_eq!(properties.track_number, Some(1));
        assert_eq!(properties.duration, Some(Duration::from_millis(84_780)));
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
    }

    #[test]
    fn registry_expands_every_track_in_a_multitrack_nsf() {
        let path =
            std::env::temp_dir().join(format!("kog-gme-multitrack-{}.nsf", std::process::id()));
        let mut nsf = std::fs::read(test_nsf_path()).expect("read official NSF fixture");
        nsf[6] = 3;
        std::fs::write(&path, nsf).expect("write multitrack NSF fixture");

        let registry = DecoderRegistry::default();
        let sources = registry
            .expand(path.clone())
            .expect("expand multitrack NSF");
        assert_eq!(sources.len(), 3);
        assert_eq!(
            sources
                .iter()
                .map(|source| source.subsong)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(1), Some(2)]
        );
        assert_eq!(sources[2].display_label(), format!("{}#3", path.display()));

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn gme_source_renders_non_silent_pcm_and_seeks() {
        let mut emu = GameMusicEmu::open(&test_nsf_path(), 44_100).expect("open NSF");
        let info = emu.track_info(0).expect("NSF metadata");
        let plan = info.playback_plan(GME_DEFAULT_LENGTH, GME_DEFAULT_FADE, GME_DEFAULT_LOOP_COUNT);
        emu.start_track(0, plan).expect("start NSF");
        let mut source = GmeSource::new(emu, 44_100, plan).expect("GME source");

        assert!(
            source
                .by_ref()
                .take(4_410 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered NSF PCM was silent"
        );
        source
            .try_seek(Duration::from_secs(1))
            .expect("seek NSF source");
        assert_eq!(source.frames_rendered, 44_100);
        assert_eq!(source.samples_emitted, 88_200);
        assert!(
            source
                .by_ref()
                .take(2_205 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered NSF PCM was silent after seeking"
        );
    }

    #[test]
    fn gme_routing_stays_separate_from_cogs_other_native_families() {
        let registry = DecoderRegistry::default();
        assert_eq!(
            registry.backend_id_for(Path::new("song.nsf")),
            Some("game-music-emu")
        );
        assert_eq!(
            registry.backend_id_for(Path::new("song.sfm")),
            Some("cog-gme-sfm")
        );
        assert_eq!(
            registry.backend_id_for(Path::new("song.vgm")),
            Some("libvgm")
        );
    }
}
