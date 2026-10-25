//! KSS rips (MSX and Sega 8-bit sound drivers) played through libkss.
//!
//! Game Music Emu still reads the song list, names and lengths (including a
//! companion .m3u), but libkss makes the sound: GME leaves out MSX-MUSIC and
//! MSX-AUDIO entirely, and runs some Game Gear drivers' code wrong (Phantasy
//! Star Adventure's drums).

use std::ffi::c_void;
use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::gme::GmePlaybackPlan;
use crate::gme_decoder::GmeBackend;

const KSS_EXTENSIONS: &[&str] = &["kss"];
const KSS_SAMPLE_RATE: u32 = 44_100;
const KSS_CHANNELS: u16 = 2;
const KSS_RENDER_FRAMES: usize = 1_024;
/// libkss mixes well below full scale; in 0.1875 dB steps, +9 dB keeps the
/// loudest MSX FM rips (4th Unit 3) clear of clipping.
const KSS_MASTER_VOLUME: i32 = 48;

unsafe extern "C" {
    fn kog_kss_open(
        data: *const u8,
        size: usize,
        rate: u32,
        song: i32,
        master_volume: i32,
        error: *mut u8,
        error_size: usize,
    ) -> *mut c_void;
    fn kog_kss_render(song: *mut c_void, output: *mut f32, frames: i32) -> std::ffi::c_long;
    fn kog_kss_skip(song: *mut c_void, frames: std::ffi::c_ulong);
    fn kog_kss_close(song: *mut c_void);
}

pub struct KssBackend;

impl DecoderBackend for KssBackend {
    fn id(&self) -> &'static str {
        "libkss"
    }

    fn display_name(&self) -> &'static str {
        "libkss (MSX / Sega 8-bit)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        KSS_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities { seek: true, subsongs: true, loop_metadata: true, companion_files: true }
    }

    fn subsong_count(&self, path: &Path) -> Result<Option<u32>, String> {
        GmeBackend.subsong_count(path)
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let mut properties = GmeBackend.probe(source)?;
        properties.sample_rate = Some(KSS_SAMPLE_RATE);
        Ok(properties)
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        let (track, plan) = GmeBackend::kss_track_and_plan(source)?;
        let song = companion_song_number(&source.path, track).unwrap_or(track);
        let data = std::fs::read(&source.path).map_err(|error| format!("reading {}: {error}", source.path.display()))?;
        crate::silence::append_trimmed(player, KssSource::new(KssSong::open(data, song)?, plan));
        Ok(())
    }
}

/// The song number the `track`th entry of the companion .m3u plays. GME
/// applies the playlist to its track list, so the backend's subsong index is
/// a playlist position, while libkss takes the KSS song number itself.
fn companion_song_number(path: &Path, track: u32) -> Option<u32> {
    let text = std::fs::read(path.with_extension("m3u")).ok()?;
    let text = kog_core::text_encoding::decode(&text);
    let entry = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .nth(usize::try_from(track).ok()?)?;
    // name.kss::KSS,<song>,<title>,...  (song in decimal or $hex)
    let (_, fields) = entry.split_once("::")?;
    let number = fields.split(',').nth(1)?.trim();
    match number.strip_prefix('$') {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => number.parse().ok(),
    }
}

struct KssSong {
    handle: NonNull<c_void>,
    data: Vec<u8>,
    song: u32,
}

unsafe impl Send for KssSong {}

impl KssSong {
    fn open(data: Vec<u8>, song: u32) -> Result<Self, String> {
        let handle = Self::start(&data, song)?;
        Ok(Self { handle, data, song })
    }

    fn start(data: &[u8], song: u32) -> Result<NonNull<c_void>, String> {
        let mut error = [0_u8; 256];
        let song_number = i32::try_from(song).map_err(|_| "KSS song number is too large".to_owned())?;
        let handle = unsafe {
            kog_kss_open(data.as_ptr(), data.len(), KSS_SAMPLE_RATE, song_number, KSS_MASTER_VOLUME, error.as_mut_ptr(), error.len())
        };
        NonNull::new(handle).ok_or_else(|| {
            let end = error.iter().position(|byte| *byte == 0).unwrap_or(error.len());
            let message = String::from_utf8_lossy(&error[..end]).into_owned();
            if message.is_empty() { "libkss could not open this file".to_owned() } else { message }
        })
    }

    fn render(&mut self, output: &mut [f32]) -> usize {
        let frames = i32::try_from(output.len() / 2).unwrap_or(i32::MAX);
        let rendered = unsafe { kog_kss_render(self.handle.as_ptr(), output.as_mut_ptr(), frames) };
        usize::try_from(rendered).unwrap_or(0)
    }

    /// libkss cannot jump; start the song again and run it to `frames`.
    fn restart_at(&mut self, frames: u64) -> Result<(), String> {
        let fresh = Self::start(&self.data, self.song)?;
        unsafe { kog_kss_close(self.handle.as_ptr()) };
        self.handle = fresh;
        unsafe { kog_kss_skip(self.handle.as_ptr(), std::ffi::c_ulong::try_from(frames).unwrap_or(std::ffi::c_ulong::MAX)) };
        Ok(())
    }
}

impl Drop for KssSong {
    fn drop(&mut self) {
        unsafe { kog_kss_close(self.handle.as_ptr()) };
    }
}

struct KssSource {
    song: KssSong,
    /// Frames before the fade starts, and in total.
    fade_from: u64,
    total: u64,
    position: u64,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl KssSource {
    fn new(song: KssSong, plan: GmePlaybackPlan) -> Self {
        let frames = |ms: u64| ms.saturating_mul(u64::from(KSS_SAMPLE_RATE)) / 1_000;
        Self {
            song,
            fade_from: frames(plan.play_length_ms),
            total: frames(plan.total_length_ms),
            position: 0,
            pcm: vec![0.0; KSS_RENDER_FRAMES * usize::from(KSS_CHANNELS)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }
}

impl Iterator for KssSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pcm_index == self.pcm_samples {
            let left = self.total.saturating_sub(self.position);
            if left == 0 {
                return None;
            }
            let frames = usize::try_from(left).unwrap_or(usize::MAX).min(KSS_RENDER_FRAMES);
            let channels = usize::from(KSS_CHANNELS);
            let rendered = self.song.render(&mut self.pcm[..frames * channels]);
            if rendered == 0 {
                return None;
            }
            let fade_length = self.total.saturating_sub(self.fade_from).max(1) as f32;
            for frame in 0..rendered {
                let at = self.position + frame as u64;
                if at >= self.fade_from {
                    let gain = 1.0 - (at - self.fade_from) as f32 / fade_length;
                    for sample in &mut self.pcm[frame * channels..(frame + 1) * channels] {
                        *sample *= gain.max(0.0);
                    }
                }
            }
            self.position += rendered as u64;
            self.pcm_index = 0;
            self.pcm_samples = rendered * channels;
        }
        let sample = self.pcm[self.pcm_index];
        self.pcm_index += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Source for KssSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(KSS_CHANNELS).expect("KSS output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(KSS_SAMPLE_RATE).expect("KSS sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.total as f64 / f64::from(KSS_SAMPLE_RATE)))
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        let frames = ((position.as_secs_f64() * f64::from(KSS_SAMPLE_RATE)) as u64).min(self.total);
        self.song
            .restart_at(frames)
            .map_err(|error| SeekError::Other(std::sync::Arc::new(std::io::Error::other(error))))?;
        self.position = frames;
        self.pcm_samples = 0;
        self.pcm_index = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companion_playlist_entries_name_the_kss_song() {
        let dir = std::env::temp_dir().join(format!("kog-kss-m3u-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let kss = dir.join("G-3216.kss");
        std::fs::write(
            dir.join("G-3216.m3u"),
            "# @TITLE x\n\nG-3216.kss::KSS,0,Title,1:17,,10\nG-3216.kss::KSS,2,Name Entry,1:17,,10\nG-3216.kss::KSS,$0A,Hex,0:10\n",
        )
        .unwrap();
        assert_eq!(companion_song_number(&kss, 0), Some(0));
        assert_eq!(companion_song_number(&kss, 1), Some(2));
        assert_eq!(companion_song_number(&kss, 2), Some(10));
        assert_eq!(companion_song_number(&kss, 3), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod probe {
    use super::*;

    /// Renders KSS songs through libkss and reports their level:
    ///   KOG_KSS_PROBE="<file.kss>:<song>|..." cargo test -p kog-audio --lib -- --ignored kss_probe --nocapture
    #[test]
    #[ignore = "needs KOG_KSS_PROBE"]
    fn kss_probe() {
        for item in std::env::var("KOG_KSS_PROBE").expect("set KOG_KSS_PROBE").split('|') {
            let (path, song) = item.rsplit_once(':').expect("path:song");
            let mut kss = KssSong::open(std::fs::read(path).unwrap(), song.parse().unwrap()).unwrap();
            let mut pcm = vec![0.0_f32; 2 * 44_100 * 10];
            let frames = kss.render(&mut pcm);
            let peak = pcm.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
            let rms = (pcm.iter().map(|s| s * s).sum::<f32>() / pcm.len() as f32).sqrt();
            println!("{frames} frames peak {peak:.3} rms {rms:.3} {path}:{song}");
        }
    }
}
