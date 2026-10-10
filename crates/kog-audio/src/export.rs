//! Export tracks to audio files: decode with the same decoders and settings
//! as playback, optionally through the equalizer and effects, and encode with
//! the linked FFmpeg into WAV, FLAC, ALAC, MP3, AAC, Vorbis or Opus.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::io::Read;
use std::num::{NonZeroU16, NonZeroU32};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use kog_core::effects::{EffectsControl, EffectsSettings, EffectsSource};
use kog_core::equalizer::{EqualizerControl, EqualizerSettings, EqualizerSource};
use rodio::Source;
use rodio::source::SeekError;

use crate::decoder::{DecoderSettings, PlaybackSource};
use crate::streaming::PcmReader;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Wav16,
    Wav24,
    Flac,
    Alac,
    Mp3,
    Aac,
    Vorbis,
    Opus,
}

pub const EXPORT_FORMATS: [ExportFormat; 8] = [
    ExportFormat::Flac,
    ExportFormat::Wav16,
    ExportFormat::Wav24,
    ExportFormat::Alac,
    ExportFormat::Mp3,
    ExportFormat::Aac,
    ExportFormat::Vorbis,
    ExportFormat::Opus,
];

impl ExportFormat {
    pub fn id(self) -> &'static str {
        match self {
            Self::Wav16 => "wav",
            Self::Wav24 => "wav24",
            Self::Flac => "flac",
            Self::Alac => "alac",
            Self::Mp3 => "mp3",
            Self::Aac => "m4a",
            Self::Vorbis => "ogg",
            Self::Opus => "opus",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Wav16 => "WAV (16-bit)",
            Self::Wav24 => "WAV (24-bit)",
            Self::Flac => "FLAC (lossless)",
            Self::Alac => "Apple Lossless (M4A)",
            Self::Mp3 => "MP3",
            Self::Aac => "AAC (M4A)",
            Self::Vorbis => "Ogg Vorbis",
            Self::Opus => "Opus",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav16 | Self::Wav24 => "wav",
            Self::Flac => "flac",
            Self::Alac | Self::Aac => "m4a",
            Self::Mp3 => "mp3",
            Self::Vorbis => "ogg",
            Self::Opus => "opus",
        }
    }

    /// Lossy formats take a bitrate; lossless ones ignore it.
    pub fn lossy(self) -> bool {
        matches!(self, Self::Mp3 | Self::Aac | Self::Vorbis | Self::Opus)
    }

    pub fn default_bitrate(self) -> u16 {
        match self {
            Self::Opus => 160,
            Self::Vorbis | Self::Aac => 256,
            Self::Mp3 => 320,
            _ => 0,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        EXPORT_FORMATS.into_iter().find(|format| format.id() == id)
    }

    fn native_id(self) -> c_int {
        match self {
            Self::Wav16 => 3,
            Self::Wav24 => 4,
            Self::Flac => 5,
            Self::Alac => 6,
            Self::Mp3 => 7,
            Self::Aac => 8,
            Self::Vorbis => 9,
            Self::Opus => 10,
        }
    }
}

/// How tracks are exported.
#[derive(Clone)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub bitrate_kbps: u16,
    /// Run the audio through the equalizer and effects, as playback does.
    pub apply_effects: bool,
    pub equalizer: EqualizerSettings,
    pub effects: EffectsSettings,
    pub decoder: DecoderSettings,
    /// Also write an M3U playlist of the exported files, with this name.
    pub playlist: Option<String>,
}

/// One track to export and the tags to write into it.
#[derive(Clone, Debug, Default)]
pub struct ExportTrack {
    pub source: PlaybackSource,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: Option<u32>,
}

/// Progress of an export, readable from another thread.
#[derive(Default)]
pub struct ExportProgress {
    pub done_ms: AtomicU64,
    pub total_ms: AtomicU64,
    pub cancel: AtomicBool,
}

/// A file name for a track: its number and title, without characters that
/// file systems reject.
pub fn file_name(track: &ExportTrack, format: ExportFormat) -> String {
    let stem = if track.title.trim().is_empty() {
        track.source.path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_else(|| "Track".into())
    } else {
        track.title.trim().to_owned()
    };
    let stem = match track.track_number {
        Some(number) if !track.title.trim().is_empty() => format!("{number:02} {stem}"),
        _ => stem,
    };
    let safe: String = stem
        .chars()
        .map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let safe = safe.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let safe: String = if safe.is_empty() { "Track".into() } else { safe.chars().take(180).collect() };
    format!("{safe}.{}", format.extension())
}

/// A path in `folder` that does not overwrite an existing file.
pub fn unused_path(folder: &Path, name: &str) -> PathBuf {
    let path = folder.join(name);
    if !path.exists() {
        return path;
    }
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    (2..)
        .map(|n| folder.join(format!("{stem} ({n}).{extension}")))
        .find(|path| !path.exists())
        .expect("some numbered name is free")
}

/// Decoded PCM as a rodio source, so the equalizer and effects can run on it.
struct PcmSource {
    reader: PcmReader,
    buffer: Vec<u8>,
    cursor: usize,
    filled: usize,
    channels: NonZeroU16,
    rate: NonZeroU32,
}

impl Iterator for PcmSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.cursor + 4 > self.filled {
            let keep = self.filled - self.cursor;
            self.buffer.copy_within(self.cursor..self.filled, 0);
            self.filled = keep;
            self.cursor = 0;
            while self.filled < 4 {
                let count = self.reader.read(&mut self.buffer[self.filled..]).ok()?;
                if count == 0 {
                    return None;
                }
                self.filled += count;
            }
        }
        let sample = f32::from_le_bytes(self.buffer[self.cursor..self.cursor + 4].try_into().ok()?);
        self.cursor += 4;
        Some(sample)
    }
}

impl Source for PcmSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> NonZeroU16 {
        self.channels
    }

    fn sample_rate(&self) -> NonZeroU32 {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }

    fn try_seek(&mut self, _: Duration) -> Result<(), SeekError> {
        Err(SeekError::NotSupported { underlying_source: "export" })
    }
}

unsafe extern "C" {
    fn kog_ffmpeg_encoder_open_file(
        format: c_int,
        bitrate_kbps: c_int,
        input_rate: c_int,
        channels: c_int,
        path: *const c_char,
        tags: *const *const c_char,
        error: *mut c_char,
        error_capacity: usize,
    ) -> *mut c_void;
    fn kog_ffmpeg_encoder_push(encoder: *mut c_void, interleaved: *const f32, frames: c_int) -> c_int;
    fn kog_ffmpeg_encoder_finish(encoder: *mut c_void) -> c_int;
    fn kog_ffmpeg_encoder_error(encoder: *const c_void) -> *const c_char;
    fn kog_ffmpeg_encoder_close(encoder: *mut c_void);
}

struct FileEncoder(NonNull<c_void>);

impl FileEncoder {
    fn error(&self) -> String {
        unsafe { CStr::from_ptr(kog_ffmpeg_encoder_error(self.0.as_ptr())) }.to_string_lossy().into_owned()
    }
}

impl Drop for FileEncoder {
    fn drop(&mut self) {
        unsafe { kog_ffmpeg_encoder_close(self.0.as_ptr()) }
    }
}

/// Export one track to `path`. On failure or cancel the partial file is
/// removed.
pub fn export_track(track: &ExportTrack, options: &ExportOptions, path: &Path, progress: &ExportProgress) -> Result<(), String> {
    let result = export_inner(track, options, path, progress);
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

fn export_inner(track: &ExportTrack, options: &ExportOptions, path: &Path, progress: &ExportProgress) -> Result<(), String> {
    let reader = PcmReader::open(track.source.clone(), options.decoder.clone())?;
    let rate = reader.sample_rate();
    let channels = reader.channels();
    let total = reader.duration().unwrap_or(Duration::from_secs(600));
    progress.total_ms.fetch_add(total.as_millis() as u64, Ordering::Relaxed);
    let pcm = PcmSource {
        reader,
        buffer: vec![0; 64 * 1024],
        cursor: 0,
        filled: 0,
        channels: NonZeroU16::new(channels).ok_or("no audio channels")?,
        rate: NonZeroU32::new(rate).ok_or("no sample rate")?,
    };
    let mut samples: Box<dyn Iterator<Item = f32>> = if options.apply_effects {
        Box::new(EffectsSource::new(
            EqualizerSource::new(pcm, EqualizerControl::new(options.equalizer.clone())),
            EffectsControl::new(options.effects.clone()),
        ))
    } else {
        Box::new(pcm)
    };

    let path_text = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| "the file name contains a null byte")?;
    let tag = |value: &str| CString::new(value.replace('\0', "")).unwrap_or_default();
    let mut pairs: Vec<(CString, CString)> = Vec::new();
    for (key, value) in [("title", &track.title), ("artist", &track.artist), ("album", &track.album)] {
        if !value.trim().is_empty() {
            pairs.push((tag(key), tag(value)));
        }
    }
    if let Some(number) = track.track_number {
        pairs.push((tag("track"), tag(&number.to_string())));
    }
    pairs.push((tag("encoder"), tag("Kog")));
    let mut tags: Vec<*const c_char> = pairs.iter().flat_map(|(key, value)| [key.as_ptr(), value.as_ptr()]).collect();
    tags.push(std::ptr::null());
    tags.push(std::ptr::null());

    let bitrate = if options.format.lossy() {
        if options.bitrate_kbps == 0 { options.format.default_bitrate() } else { options.bitrate_kbps.clamp(32, 512) }
    } else {
        0
    };
    let mut message = [0 as c_char; 1024];
    let raw = unsafe {
        kog_ffmpeg_encoder_open_file(
            options.format.native_id(),
            c_int::from(bitrate),
            c_int::try_from(rate).map_err(|_| "sample rate is too large")?,
            c_int::from(channels),
            path_text.as_ptr(),
            tags.as_ptr(),
            message.as_mut_ptr(),
            message.len(),
        )
    };
    let encoder = NonNull::new(raw)
        .map(FileEncoder)
        .ok_or_else(|| unsafe { CStr::from_ptr(message.as_ptr()) }.to_string_lossy().into_owned())?;

    let frame = usize::from(channels);
    let mut chunk = Vec::with_capacity(4096 * frame);
    let mut frames_done = 0u64;
    loop {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err("Export cancelled".into());
        }
        chunk.clear();
        chunk.extend(samples.by_ref().take(4096 * frame));
        let frames = chunk.len() / frame;
        if frames == 0 {
            break;
        }
        let status = unsafe { kog_ffmpeg_encoder_push(encoder.0.as_ptr(), chunk.as_ptr(), frames as c_int) };
        if status != 0 {
            return Err(encoder.error());
        }
        frames_done += frames as u64;
        let before = (frames_done - frames as u64) * 1000 / u64::from(rate);
        let after = frames_done * 1000 / u64::from(rate);
        progress.done_ms.fetch_add(after - before, Ordering::Relaxed);
    }
    if unsafe { kog_ffmpeg_encoder_finish(encoder.0.as_ptr()) } != 0 {
        return Err(encoder.error());
    }
    // Count the whole track as done even if it ended early.
    let total_ms = total.as_millis() as u64;
    let done_ms = frames_done * 1000 / u64::from(rate);
    if total_ms > done_ms {
        progress.done_ms.fetch_add(total_ms - done_ms, Ordering::Relaxed);
    }
    Ok(())
}

/// Export tracks one after another into `folder`; returns the files written
/// and the first error per failed track.
pub fn export_tracks(
    tracks: &[ExportTrack],
    options: &ExportOptions,
    folder: &Path,
    progress: Arc<ExportProgress>,
) -> (Vec<PathBuf>, Vec<String>) {
    let mut written = Vec::new();
    let mut titles = Vec::new();
    let mut errors = Vec::new();
    if let Err(error) = std::fs::create_dir_all(folder) {
        errors.push(format!("{}: {error}", folder.display()));
        return (written, errors);
    }
    for track in tracks {
        if progress.cancel.load(Ordering::Relaxed) {
            break;
        }
        let path = unused_path(folder, &file_name(track, options.format));
        match export_track(track, options, &path, &progress) {
            Ok(()) => {
                titles.push(track.title.clone());
                written.push(path);
            }
            Err(error) => errors.push(format!("{}: {error}", track.source.path.display())),
        }
    }
    if let Some(name) = options.playlist.as_deref().filter(|_| !written.is_empty()) {
        let list = ExportTrack { title: name.to_owned(), ..ExportTrack::default() };
        let path = unused_path(folder, &file_name(&list, options.format).replace(&format!(".{}", options.format.extension()), ".m3u"));
        let mut text = String::from("#EXTM3U\n");
        for (file, title) in written.iter().zip(&titles) {
            let name = file.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            text.push_str(&format!("#EXTINF:-1,{}\n{name}\n", if title.is_empty() { &name } else { title }));
        }
        match std::fs::write(&path, text) {
            Ok(()) => written.push(path),
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    (written, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe_and_numbered() {
        let track = ExportTrack {
            source: PlaybackSource { path: "/music/song.nsf".into(), ..PlaybackSource::default() },
            title: "Boss: Round 1/2?".into(),
            track_number: Some(3),
            ..ExportTrack::default()
        };
        assert_eq!(file_name(&track, ExportFormat::Flac), "03 Boss_ Round 1_2_.flac");
        let untitled = ExportTrack { title: String::new(), ..track };
        assert_eq!(file_name(&untitled, ExportFormat::Mp3), "song.mp3");
    }

    #[test]
    fn every_format_writes_a_file_that_plays_back() {
        let song = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native/game-music-emu/test.nsf");
        let folder = tempfile::tempdir().unwrap();
        let tracks: Vec<_> = (0..1)
            .map(|_| ExportTrack {
                source: PlaybackSource { path: song.clone(), ..PlaybackSource::default() },
                title: "Test".into(),
                artist: "Kog".into(),
                ..ExportTrack::default()
            })
            .collect();
        for format in EXPORT_FORMATS {
            let options = ExportOptions {
                format,
                bitrate_kbps: 0,
                apply_effects: true,
                equalizer: EqualizerSettings::default(),
                effects: kog_core::effects::presets()[2].clone(),
                decoder: DecoderSettings::default(),
                playlist: Some("Mix".into()),
            };
            let progress = Arc::new(ExportProgress::default());
            let (written, errors) = export_tracks(&tracks, &options, &folder.path().join(format.id()), progress.clone());
            assert!(errors.is_empty(), "{format:?}: {errors:?}");
            let list = std::fs::read_to_string(written.last().unwrap()).unwrap();
            assert!(list.starts_with("#EXTM3U") && list.contains(&format!("Test.{}", format.extension())), "{list}");
            let path = &written[0];
            assert_eq!(path.extension().unwrap(), format.extension());
            if format == ExportFormat::Opus {
                // Kog's own players do not decode Opus files yet; check the
                // stream header and that a whole song's worth was written.
                let bytes = std::fs::read(path).unwrap();
                assert!(bytes.starts_with(b"OggS") && bytes.windows(8).any(|w| w == b"OpusHead"));
                assert!(bytes.len() > 500_000, "Opus wrote {} bytes", bytes.len());
            } else {
                let reader = PcmReader::open_path(path.clone(), DecoderSettings::default())
                    .unwrap_or_else(|error| panic!("{format:?} does not open: {error}"));
                let seconds = reader.duration().map_or(0.0, |d| d.as_secs_f64());
                assert!(seconds > 60.0, "{format:?} lasts {seconds} s");
            }
            assert!(progress.done_ms.load(Ordering::Relaxed) >= progress.total_ms.load(Ordering::Relaxed));
        }
    }
}
