//! KSS rips (MSX and Sega 8-bit sound drivers), played through libkss.
//!
//! The song list, names and lengths come from a companion .m3u in the
//! NEZplug format the rips ship with (`name.kss::KSS,$05,Title,1:30,,10`),
//! or from the KSS header's song range when there is none.

use std::ffi::c_void;
use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};

const KSS_EXTENSIONS: &[&str] = &["kss"];
const KSS_SAMPLE_RATE: u32 = 44_100;
const KSS_CHANNELS: u16 = 2;
const KSS_RENDER_FRAMES: usize = 1_024;
/// libkss mixes well below full scale; in 0.1875 dB steps, +9 dB keeps the
/// loudest MSX FM rips (4th Unit 3) clear of clipping.
const KSS_MASTER_VOLUME: i32 = 48;
/// A plain KSS file has no song count (any of 256 numbers may play); real
/// rips hold a handful, so list this many rather than flood the pane.
const KSS_UNLISTED_SONGS: u32 = 16;

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
    fn kog_kss_inspection(song: *mut c_void, voices: *mut crate::inspection::native::Voice, capacity: usize) -> usize;
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
        Ok(Some(KssList::read(path)?.len()))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let list = KssList::read(&source.path)?;
        let track = source.subsong.unwrap_or(0);
        let song = list.song(track, &source.path)?;
        let plan = song.plan();
        let nonempty = |text: &str| (!text.is_empty()).then(|| text.to_owned());
        Ok(StreamProperties {
            duration: Some(Duration::from_millis(plan.total_ms)),
            sample_rate: Some(KSS_SAMPLE_RATE),
            channels: Some(KSS_CHANNELS),
            title: nonempty(&song.title),
            artist: nonempty(&list.artist),
            album: nonempty(&list.album),
            genre: Some(list.system.to_owned()),
            track_number: Some(track + 1),
            codec: Some(format!("KSS ({})", list.system)),
            ..StreamProperties::default()
        })
    }

    fn append_observed(&self, source: &PlaybackSource, player: &Player, monitor: &crate::inspection::Monitor) -> Result<(), String> {
        let mut audio = Self::open(source)?;
        monitor.describe(self.display_name(), "registers", "Writes to the KSS sound chips: tone frequencies, key gates, instruments, Game Gear panning and FM rhythm. Levels are programmed volumes, not the chips' envelope output.");
        audio.inspection = Some(monitor.producer(KSS_SAMPLE_RATE));
        crate::silence::append_trimmed(player, audio);
        Ok(())
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        crate::silence::append_trimmed(player, Self::open(source)?);
        Ok(())
    }
}

impl KssBackend {
    fn open(source: &PlaybackSource) -> Result<KssSource, String> {
        let list = KssList::read(&source.path)?;
        let song = list.song(source.subsong.unwrap_or(0), &source.path)?;
        let data = std::fs::read(&source.path).map_err(|error| format!("reading {}: {error}", source.path.display()))?;
        Ok(KssSource::new(KssSong::open(data, song.number)?, song.plan()))
    }
}

/// One playable song: its KSS song number and what the playlist says of it.
#[derive(Clone, Debug, Default, PartialEq)]
struct KssEntry {
    number: u32,
    title: String,
    length_ms: Option<u64>,
    intro_ms: Option<u64>,
    loop_ms: Option<u64>,
    fade_ms: Option<u64>,
}

struct KssPlan {
    play_ms: u64,
    total_ms: u64,
}

impl KssEntry {
    fn plan(&self) -> KssPlan {
        let millis = |duration: Duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        // A known loop plays as many times as the loop policy asks; a listed
        // length otherwise; songs with neither play the default length.
        let policy = crate::looping::current();
        let play_ms = match (self.length_ms, self.loop_ms) {
            (_, Some(looped)) if looped > 0 => {
                let intro = self.intro_ms.unwrap_or(0);
                let loops = policy.loops(Duration::from_millis(intro), Duration::from_millis(looped));
                intro + looped * u64::from(loops)
            }
            (Some(length), _) if length > 0 => length,
            _ => millis(policy.untimed_length()),
        };
        let fade_ms = self.fade_ms.unwrap_or_else(|| millis(policy.fade()));
        KssPlan { play_ms, total_ms: play_ms.saturating_add(fade_ms) }
    }
}

struct KssList {
    entries: Vec<KssEntry>,
    album: String,
    artist: String,
    system: &'static str,
}

impl KssList {
    fn read(path: &Path) -> Result<Self, String> {
        let header = read_header(path)?;
        let system = if header[0x0f] & 0x02 != 0 { "Sega 8-bit" } else { "MSX" };
        let mut list = match std::fs::read(path.with_extension("m3u")) {
            Ok(bytes) => parse_playlist(&kog_core::text_encoding::decode(&bytes)),
            Err(_) => Self { entries: Vec::new(), album: String::new(), artist: String::new(), system },
        };
        list.system = system;
        if list.entries.is_empty() {
            // KSSX headers name the song range; KSCC ones say nothing.
            let (first, count) = if &header[..4] == b"KSSX" && header[0x0e] >= 0x10 {
                let first = u32::from(u16::from_le_bytes([header[0x18], header[0x19]]));
                let last = u32::from(u16::from_le_bytes([header[0x1a], header[0x1b]]));
                (first, last.checked_sub(first).map_or(KSS_UNLISTED_SONGS, |span| (span + 1).min(256)))
            } else {
                (0, KSS_UNLISTED_SONGS)
            };
            list.entries = (first..first + count).map(|number| KssEntry { number, ..KssEntry::default() }).collect();
        }
        Ok(list)
    }

    fn len(&self) -> u32 {
        u32::try_from(self.entries.len()).unwrap_or(u32::MAX)
    }

    fn song(&self, track: u32, path: &Path) -> Result<&KssEntry, String> {
        self.entries
            .get(usize::try_from(track).unwrap_or(usize::MAX))
            .ok_or_else(|| format!("{} has no song {} (it lists {})", path.display(), track + 1, self.entries.len()))
    }
}

fn read_header(path: &Path) -> Result<[u8; 0x20], String> {
    use std::io::Read;
    let mut header = [0_u8; 0x20];
    let mut file = std::fs::File::open(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
    let read = file.read(&mut header).map_err(|error| format!("reading {}: {error}", path.display()))?;
    if read < 0x10 || !(&header[..4] == b"KSCC" || &header[..4] == b"KSSX") {
        return Err(format!("{} is not a KSS file", path.display()));
    }
    Ok(header)
}

/// Read a NEZplug-style playlist: `# @TITLE`/`# @ARTIST` comments, then
/// `file::KSS,<song>,<title>,<time>,<loop>,<fade>` lines (song in decimal or
/// `$hex`; times as `[h:]m:ss[.ms]`; `\,` escapes a comma in a title).
fn parse_playlist(text: &str) -> KssList {
    let mut list = KssList { entries: Vec::new(), album: String::new(), artist: String::new(), system: "MSX" };
    let mut composer = String::new();
    for line in text.lines().map(str::trim) {
        if let Some(comment) = line.strip_prefix('#') {
            let comment = comment.trim();
            if let Some((key, value)) = comment.strip_prefix('@').and_then(|rest| rest.split_once(char::is_whitespace)) {
                let value = value.trim().to_owned();
                match key {
                    "TITLE" => list.album = value,
                    "ARTIST" => list.artist = value,
                    "COMPOSER" => composer = value,
                    _ => {}
                }
            }
            continue;
        }
        let Some((_, fields)) = line.split_once("::") else { continue };
        let fields = split_fields(fields);
        if !fields.first().is_some_and(|kind| kind.eq_ignore_ascii_case("KSS")) {
            continue;
        }
        let Some(number) = fields.get(1).and_then(|field| parse_number(field)) else { continue };
        let field = |index: usize| fields.get(index).map(String::as_str).unwrap_or_default();
        let length_ms = parse_time(field(3));
        let (intro_ms, loop_ms) = match field(4).trim() {
            "-" => (None, length_ms),
            looped => match looped.strip_suffix('-').map(parse_time) {
                // "1:00-": the intro's length, the rest of the song loops.
                Some(intro) => (intro, length_ms.zip(intro).map(|(length, intro)| length.saturating_sub(intro))),
                None => (None, parse_time(looped)),
            },
        };
        list.entries.push(KssEntry {
            number,
            title: field(2).trim().to_owned(),
            length_ms,
            intro_ms,
            loop_ms,
            fade_ms: parse_time(field(5)),
        });
    }
    if list.artist.is_empty() {
        list.artist = composer;
    }
    list
}

fn split_fields(text: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => fields.last_mut().expect("one field").extend(chars.next()),
            ',' => fields.push(String::new()),
            c => fields.last_mut().expect("one field").push(c),
        }
    }
    fields
}

fn parse_number(field: &str) -> Option<u32> {
    let field = field.trim();
    match field.strip_prefix('$') {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => field.parse().ok(),
    }
}

fn parse_time(field: &str) -> Option<u64> {
    let field = field.trim();
    if field.is_empty() {
        return None;
    }
    let (clock, millis) = field.split_once('.').unwrap_or((field, ""));
    let mut seconds = 0_u64;
    for part in clock.split(':') {
        seconds = seconds.checked_mul(60)?.checked_add(part.trim().parse().ok()?)?;
    }
    let millis = if millis.is_empty() {
        0
    } else {
        // ".5" is half a second, ".05" fifty milliseconds.
        let digits: String = millis.chars().take(3).collect();
        digits.parse::<u64>().ok()? * 10_u64.pow(3 - digits.len() as u32)
    };
    Some(seconds * 1_000 + millis)
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

    fn inspection(&mut self) -> crate::inspection::FrameData {
        let mut voices = vec![crate::inspection::native::Voice::default(); 64];
        let count = unsafe { kog_kss_inspection(self.handle.as_ptr(), voices.as_mut_ptr(), voices.len()) }.min(voices.len());
        crate::inspection::native::frame(&voices[..count])
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
    inspection: Option<crate::inspection::Producer>,
    /// Frames before the fade starts, and in total.
    fade_from: u64,
    total: u64,
    position: u64,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl KssSource {
    fn new(song: KssSong, plan: KssPlan) -> Self {
        let frames = |ms: u64| ms.saturating_mul(u64::from(KSS_SAMPLE_RATE)) / 1_000;
        Self {
            song,
            inspection: None,
            fade_from: frames(plan.play_ms),
            total: frames(plan.total_ms),
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
            let frames = usize::try_from(left)
                .unwrap_or(usize::MAX)
                .min(self.inspection.as_ref().map_or(KSS_RENDER_FRAMES, |p| p.render_frames(KSS_RENDER_FRAMES)));
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
            if let Some(producer) = &mut self.inspection {
                producer.advance(rendered);
                if producer.enabled() {
                    producer.publish_offset(self.song.inspection(), 0.0);
                }
            }
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
        if let Some(producer) = &mut self.inspection {
            producer.seek(Duration::from_secs_f64(frames as f64 / f64::from(KSS_SAMPLE_RATE)));
        }
        self.pcm_samples = 0;
        self.pcm_index = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kss_without_a_playlist_lists_sixteen_songs() {
        let temporary = tempfile::tempdir().expect("temporary music");
        let path = temporary.path().join("synthetic.kss");
        let mut bytes = vec![0_u8; 16];
        bytes[0..4].copy_from_slice(b"KSCC");
        bytes.extend_from_slice(&[0xC9; 8]);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(KssBackend.subsong_count(&path).unwrap(), Some(KSS_UNLISTED_SONGS));
        let list = KssList::read(&path).unwrap();
        assert_eq!(list.entries[15].number, 15);
        assert_eq!(list.system, "MSX");
    }

    #[test]
    fn playlists_give_song_numbers_titles_and_lengths() {
        let list = parse_playlist(
            "# @TITLE       Phantasy Star Adventure\n# @COMPOSER    Someone\n\n\
             G-3216.kss::KSS,0,Title,1:17,,10\n\
             G-3216.kss::KSS,$0A,Name\\, Entry,0:30.5,0:10-,\n\
             G-3216.kss::KSS,3,Battle,,1:00,\n\
             not a song line\n",
        );
        assert_eq!(list.album, "Phantasy Star Adventure");
        assert_eq!(list.artist, "Someone");
        assert_eq!(list.entries.len(), 3);
        assert_eq!((list.entries[0].number, list.entries[0].title.as_str()), (0, "Title"));
        let title = list.entries[0].plan();
        assert_eq!((title.play_ms, title.total_ms), (77_000, 87_000));
        assert_eq!((list.entries[1].number, list.entries[1].title.as_str()), (10, "Name, Entry"));
        assert_eq!((list.entries[1].intro_ms, list.entries[1].loop_ms), (Some(10_000), Some(20_500)));
        // A known loop wins over the listed length: the intro plus the loop twice.
        assert_eq!(list.entries[1].plan().play_ms, 51_000);
        // No length: the intro plus the loop twice.
        assert_eq!(list.entries[2].plan().play_ms, 120_000);
        assert_eq!(list.entries[2].plan().total_ms, 128_000);
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
            let mut voices = vec![crate::inspection::native::Voice::default(); 64];
            let count = unsafe { kog_kss_inspection(kss.handle.as_ptr(), voices.as_mut_ptr(), voices.len()) };
            let text = |bytes: &[u8]| String::from_utf8_lossy(&bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]).into_owned();
            for voice in &voices[..count] {
                println!("  {} active={} key={:.1} level={:.2} pan={} [{}] {}", text(&voice.name), voice.active, voice.key, voice.level, voice.pan, text(&voice.instrument), text(&voice.details));
            }
        }
    }
}
