//! Encoded-stream cache and transcoding.
//!
//! Why a cache and not just live encoding: Kog's emulator-backed decoders
//! (Nuked SC-55, vgmstream, GME, PSF) can render slower than realtime, so a
//! live transcode cannot promise 1x, and seeking would mean re-rendering from
//! the start every time. Encoding once into a per-track file fixes both: the
//! first play streams progressively as it encodes, and every later play or
//! seek is a plain file read with `Range` support.
//!
//! Encoding shell out to the pinned `ffmpeg` binary rather than linking an
//! encoder in-process. That keeps Kog's only libav usage on the decode path
//! and avoids a second ABI surface; the tradeoff is a runtime dependency on
//! the ffmpeg executable.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::StreamCodec;

/// Lossy quality in kbps. Ignored for lossless codecs.
pub const DEFAULT_BITRATE_KBPS: u16 = 192;
const MIN_BITRATE_KBPS: u16 = 48;
const MAX_BITRATE_KBPS: u16 = 320;

/// Opus is defined at 48 kHz; asking ffmpeg for another rate fails, so the
/// arg builder forces it for that codec.
const OPUS_SAMPLE_RATE: u32 = 48_000;

/// Stable identity for a cached encode: the track's locator plus what was
/// encoded. Any change to codec or bitrate is a different cache entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamKey {
    pub locator: String,
    pub codec: StreamCodec,
    pub bitrate_kbps: u16,
    /// Render settings that change the PCM, such as the selected MIDI synth.
    pub render_profile: Option<String>,
}

impl StreamKey {
    pub fn new(locator: impl Into<String>, codec: StreamCodec, bitrate_kbps: u16) -> Self {
        Self {
            locator: locator.into(),
            codec,
            bitrate_kbps: clamp_bitrate(bitrate_kbps),
            render_profile: None,
        }
    }

    pub fn with_render_profile(mut self, profile: &str) -> Self {
        self.render_profile = Some(profile.to_owned());
        self
    }

    /// File stem for this entry: a readable prefix plus a hash, so the cache
    /// directory stays browsable without colliding.
    pub fn stem(&self) -> String {
        let mut fingerprint = String::with_capacity(self.locator.len() + 16);
        fingerprint.push_str(&self.locator);
        fingerprint.push('\0');
        fingerprint.push_str(self.codec.setting_value());
        fingerprint.push('\0');
        fingerprint.push_str(&self.bitrate_kbps.to_string());
        if let Some(profile) = &self.render_profile {
            fingerprint.push('\0');
            fingerprint.push_str(profile);
        }
        let hash = fnv1a64(fingerprint.as_bytes());
        let readable: String = self
            .locator
            .rsplit('/')
            .next()
            .unwrap_or("track")
            .chars()
            .take(40)
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                    character
                } else {
                    '_'
                }
            })
            .collect();
        format!("{readable}-{hash:016x}")
    }
}

pub fn clamp_bitrate(kbps: u16) -> u16 {
    kbps.clamp(MIN_BITRATE_KBPS, MAX_BITRATE_KBPS)
}

/// On-disk cache of encoded streams, evicted oldest-first when it grows past
/// its budget.
#[derive(Clone, Debug)]
pub struct StreamCache {
    root: PathBuf,
    capacity_bytes: u64,
}

impl StreamCache {
    pub fn new(root: PathBuf, capacity_bytes: u64) -> Self {
        Self {
            root,
            capacity_bytes,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn capacity_bytes(&self) -> u64 {
        self.capacity_bytes
    }

    fn codec_dir(&self, codec: StreamCodec) -> PathBuf {
        self.root.join(codec.setting_value())
    }

    /// Where the finished encode lives.
    pub fn entry_path(&self, key: &StreamKey) -> PathBuf {
        self.codec_dir(key.codec)
            .join(format!("{}.{}", key.stem(), key.codec.extension()))
    }

    /// A sibling temp path the encoder writes into, then `commit` renames.
    /// Writing next to the destination keeps the rename atomic (same
    /// filesystem).
    pub fn partial_path(&self, key: &StreamKey) -> PathBuf {
        let entry = self.entry_path(key);
        let name = entry
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "stream".to_owned());
        entry.with_file_name(format!(".{name}.part"))
    }

    /// A finished, non-empty encode, if one exists.
    pub fn lookup(&self, key: &StreamKey) -> Option<PathBuf> {
        let path = self.entry_path(key);
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => Some(path),
            _ => None,
        }
    }

    pub fn create_partial(&self, key: &StreamKey) -> Result<std::fs::File, String> {
        let path = self.partial_path(key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("creating {}: {error}", parent.display()))?;
        }
        std::fs::File::create(&path)
            .map_err(|error| format!("creating {}: {error}", path.display()))
    }

    /// Move a finished encode into place and trim the cache. Best-effort
    /// eviction: never fails the caller for a full disk.
    pub fn commit(&self, key: &StreamKey, partial: &Path) -> Result<PathBuf, String> {
        let entry = self.entry_path(key);
        if let Some(parent) = entry.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("creating {}: {error}", parent.display()))?;
        }
        std::fs::rename(partial, &entry)
            .map_err(|error| format!("finalizing {}: {error}", entry.display()))?;
        let size = std::fs::metadata(&entry).map(|metadata| metadata.len()).unwrap_or(0);
        self.evict_to_fit(size);
        Ok(entry)
    }

    /// Total bytes currently cached.
    pub fn total_bytes(&self) -> u64 {
        self.entries()
            .iter()
            .map(|(_, size, _)| *size)
            .sum()
    }

    /// Entries as `(path, size, modified)`.
    fn entries(&self) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
        let mut found = Vec::new();
        let Ok(codecs) = std::fs::read_dir(&self.root) else {
            return found;
        };
        for codec in codecs.filter_map(Result::ok) {
            let Ok(files) = std::fs::read_dir(codec.path()) else {
                continue;
            };
            for file in files.filter_map(Result::ok) {
                let Ok(metadata) = file.metadata() else {
                    continue;
                };
                if !metadata.is_file() {
                    continue;
                }
                found.push((
                    file.path(),
                    metadata.len(),
                    metadata
                        .modified()
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                ));
            }
        }
        found
    }

    /// Remove the least recently written entries until `incoming` more bytes
    /// would fit, and clear out abandoned `.part` files while we are here.
    pub fn evict_to_fit(&self, incoming: u64) {
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|(_, size, _)| *size).sum();
        let mut partials = Vec::new();
        entries.retain(|(path, _, _)| {
            if path.extension().is_some_and(|extension| extension == "part") {
                partials.push(path.clone());
                false
            } else {
                true
            }
        });
        // Only sweep partials that have gone stale: a concurrent encode of
        // another track is writing one right now, and deleting it out from
        // under that stream makes its cache commit fail.
        const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(600);
        for partial in partials {
            let stale = std::fs::metadata(&partial)
                .and_then(|metadata| metadata.modified())
                .map(|modified| modified.elapsed().unwrap_or_default() > STALE_AFTER)
                .unwrap_or(true);
            if !stale {
                continue;
            }
            if let Ok(metadata) = std::fs::metadata(&partial) {
                total = total.saturating_sub(metadata.len());
            }
            let _ = std::fs::remove_file(partial);
        }
        if self.capacity_bytes == 0 {
            return;
        }
        if total.saturating_add(incoming) <= self.capacity_bytes {
            return;
        }
        entries.sort_by_key(|(_, _, modified)| *modified);
        for (path, size, _) in entries {
            if total.saturating_add(incoming) <= self.capacity_bytes {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(size);
            }
        }
    }

    /// Delete every cached encode. Used when settings change wholesale.
    pub fn clear(&self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The ffmpeg invocation for one codec. Separate from running it so the flags
/// can be asserted in tests without an encoder installed.
pub fn encoder_args(
    codec: StreamCodec,
    bitrate_kbps: u16,
    sample_rate: u32,
    channels: u16,
) -> Vec<String> {
    let bitrate = clamp_bitrate(bitrate_kbps);
    let input_rate = sample_rate.to_string();
    let channels = channels.to_string();
    // Raw little-endian f32 from Kog's decoder pipeline, streamed on stdin;
    // the encoded container goes to stdout so the caller can tee it into the
    // cache and the response at once.
    let mut args = vec![
        "-hide_banner".to_owned(),
        "-loglevel".to_owned(),
        "error".to_owned(),
        "-nostdin".to_owned(),
        "-f".to_owned(),
        "f32le".to_owned(),
        "-ar".to_owned(),
        input_rate,
        "-ac".to_owned(),
        channels,
        "-i".to_owned(),
        "pipe:0".to_owned(),
    ];
    match codec {
        StreamCodec::Aac => {
            args.extend([
                "-c:a".to_owned(),
                "aac".to_owned(),
                "-b:a".to_owned(),
                format!("{bitrate}k"),
                // Raw ADTS, not a container: each frame is self-describing, so
                // a client can start playing as soon as the first frame lands
                // and never needs a finalized moov. The tradeoff is that ADTS
                // carries no duration; the web UI reads that from
                // `/api/metadata` instead.
                "-f".to_owned(),
                "adts".to_owned(),
            ]);
        }
        StreamCodec::Opus => {
            args.extend([
                "-c:a".to_owned(),
                "libopus".to_owned(),
                "-b:a".to_owned(),
                format!("{bitrate}k"),
                // libopus only accepts 48 kHz.
                "-ar".to_owned(),
                OPUS_SAMPLE_RATE.to_string(),
                "-f".to_owned(),
                "ogg".to_owned(),
            ]);
        }
        StreamCodec::Flac => {
            args.extend([
                "-c:a".to_owned(),
                "flac".to_owned(),
                "-f".to_owned(),
                "flac".to_owned(),
            ]);
        }
    }
    args.extend(["-y".to_owned(), "pipe:1".to_owned()]);
    args
}

/// Encode raw f32le PCM from `input` into `output` with the given program.
/// `program` is a parameter so tests can substitute a fake encoder.
pub fn encode_to_writer(
    program: &Path,
    args: &[String],
    mut input: impl Read,
    mut output: impl Write + Send,
) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("launching {}: {error}", program.display()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "encoder stdout was not captured".to_owned())?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "encoder stdin was not captured".to_owned())?;

    // Pump stdout on a scoped second thread: an encoder can fill its output
    // pipe before it finishes reading input, which would deadlock a single
    // thread. Scoped so callers can pass borrowed writers.
    let (write_result, pump_result) = std::thread::scope(|scope| {
        let pump = scope.spawn(|| -> Result<(), String> {
            let mut stdout = stdout;
            std::io::copy(&mut stdout, &mut output)
                .map(|_| ())
                .map_err(|error| format!("reading encoded audio: {error}"))
        });
        let write_result = std::io::copy(&mut input, &mut stdin)
            .map_err(|error| format!("writing PCM to the encoder: {error}"));
        drop(stdin);
        let pump_result = pump
            .join()
            .unwrap_or_else(|_| Err("the encoder pump panicked".to_owned()));
        (write_result, pump_result)
    });

    let status = child
        .wait()
        .map_err(|error| format!("waiting for the encoder: {error}"))?;
    write_result?;
    pump_result?;
    if !status.success() {
        let mut message = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut message);
        }
        let message = message.trim();
        return Err(if message.is_empty() {
            format!("encoder exited {status}")
        } else {
            format!("encoder exited {status}: {message}")
        });
    }
    Ok(())
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache() -> (tempfile::TempDir, StreamCache) {
        let directory = tempfile::tempdir().unwrap();
        let cache = StreamCache::new(directory.path().join("streams"), 1_000);
        (directory, cache)
    }

    fn key(name: &str, codec: StreamCodec, bitrate: u16) -> StreamKey {
        StreamKey::new(name, codec, bitrate)
    }

    #[test]
    fn keys_separate_codecs_and_bitrates_but_are_stable() {
        let a = key("/music/song.flac", StreamCodec::Aac, 192);
        assert_eq!(a, key("/music/song.flac", StreamCodec::Aac, 192));
        assert_ne!(a, key("/music/song.flac", StreamCodec::Opus, 192));
        assert_ne!(a, key("/music/song.flac", StreamCodec::Aac, 128));
        assert_ne!(a, key("/music/other.flac", StreamCodec::Aac, 192));
    }

    #[test]
    fn render_profiles_separate_cached_midi_audio() {
        let basic = key("/music/song.mid", StreamCodec::Aac, 192);
        let sf2 = basic.clone().with_render_profile("rustysynth-sf2");
        let opl3 = basic.clone().with_render_profile("opl3windows");
        assert_ne!(sf2, opl3);
        assert_ne!(sf2.stem(), opl3.stem());
        assert_ne!(sf2.stem(), basic.stem());
    }

    #[test]
    fn stems_stay_readable_and_filesafe() {
        let stem = key(
            "/music/Album 01/Disc 2/track: one?.flac",
            StreamCodec::Aac,
            192,
        )
        .stem();
        assert!(stem.len() <= 60, "stem should stay short: {stem}");
        assert!(
            stem.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')),
            "stem must be filesystem safe: {stem}"
        );
        assert!(stem.contains("track"), "keeps a readable hint: {stem}");
    }

    #[test]
    fn bitrate_is_clamped_to_a_sane_range() {
        assert_eq!(clamp_bitrate(1), MIN_BITRATE_KBPS);
        assert_eq!(clamp_bitrate(9_999), MAX_BITRATE_KBPS);
        assert_eq!(clamp_bitrate(192), 192);
        assert_eq!(
            StreamKey::new("x", StreamCodec::Aac, 5).bitrate_kbps,
            MIN_BITRATE_KBPS
        );
    }

    #[test]
    fn lookup_only_returns_finished_non_empty_entries() {
        let (_dir, cache) = cache();
        let stream = key("/music/a.flac", StreamCodec::Aac, 192);
        assert_eq!(cache.lookup(&stream), None);
        cache.create_partial(&stream).unwrap().write_all(b"partial").unwrap();
        assert_eq!(cache.lookup(&stream), None, "a .part file is not an entry");
        let entry = cache.entry_path(&stream);
        std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
        std::fs::write(&entry, b"").unwrap();
        assert_eq!(cache.lookup(&stream), None, "an empty encode is not an entry");
        std::fs::write(&entry, b"encoded").unwrap();
        assert_eq!(cache.lookup(&stream), Some(entry));
    }

    #[test]
    fn commit_renames_the_partial_and_evicts_old_entries() {
        let (_dir, cache) = cache();
        // Three 400-byte entries against a 1000-byte budget: the oldest goes.
        for (index, name) in ["a", "b", "c"].iter().enumerate() {
            let stream = key(name, StreamCodec::Aac, 192);
            let partial = cache.partial_path(&stream);
            std::fs::create_dir_all(partial.parent().unwrap()).unwrap();
            std::fs::write(&partial, vec![0_u8; 400]).unwrap();
            // Distinct mtimes so eviction order is deterministic.
            let when = std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(1_000 + index as u64);
            let _ = filetime_stamp(&partial, when);
            cache.commit(&stream, &partial).unwrap();
        }
        assert!(cache.total_bytes() <= cache.capacity_bytes());
        assert_eq!(cache.lookup(&key("a", StreamCodec::Aac, 192)), None, "oldest evicted");
        assert!(cache.lookup(&key("c", StreamCodec::Aac, 192)).is_some(), "newest kept");
    }

    /// Set a file's mtime without a dependency on `filetime`.
    fn filetime_stamp(path: &Path, when: std::time::SystemTime) -> std::io::Result<()> {
        let file = std::fs::OpenOptions::new().write(true).open(path)?;
        file.set_modified(when)
    }

    #[test]
    fn an_unlimited_budget_never_evicts() {
        let (_dir, cache) = cache_with_capacity(0);
        let stream = key("a", StreamCodec::Flac, 192);
        let partial = cache.partial_path(&stream);
        std::fs::create_dir_all(partial.parent().unwrap()).unwrap();
        std::fs::write(&partial, vec![0_u8; 4_096]).unwrap();
        cache.commit(&stream, &partial).unwrap();
        assert!(cache.lookup(&stream).is_some());
    }

    fn cache_with_capacity(capacity: u64) -> (tempfile::TempDir, StreamCache) {
        let directory = tempfile::tempdir().unwrap();
        let cache = StreamCache::new(directory.path().join("streams"), capacity);
        (directory, cache)
    }

    #[test]
    fn encoder_args_select_the_right_codec_and_container() {
        let aac = encoder_args(StreamCodec::Aac, 192, 44_100, 2);
        assert!(aac.windows(2).any(|pair| pair == ["-c:a", "aac"]));
        assert!(
            aac.windows(2).any(|pair| pair == ["-f", "adts"]),
            "ADTS is the progressive, self-describing stream browsers can play"
        );
        assert!(
            !aac.iter().any(|arg| arg.contains("moov")),
            "ADTS has no container/moov"
        );
        assert!(aac.windows(2).any(|pair| pair == ["-b:a", "192k"]));
        assert_eq!(aac.last().unwrap(), "pipe:1");

        let opus = encoder_args(StreamCodec::Opus, 128, 44_100, 2);
        assert!(opus.windows(2).any(|pair| pair == ["-c:a", "libopus"]));
        assert!(opus.windows(2).any(|pair| pair == ["-f", "ogg"]));
        assert!(
            opus
                .windows(2)
                .any(|pair| pair == ["-ar", "48000"]),
            "libopus requires 48 kHz"
        );

        let flac = encoder_args(StreamCodec::Flac, 999, 44_100, 2);
        assert!(flac.windows(2).any(|pair| pair == ["-c:a", "flac"]));
        assert!(
            !flac.iter().any(|arg| arg.ends_with('k')),
            "lossless ignores a bitrate"
        );
    }

    #[test]
    #[cfg(unix)]
    fn encoding_pipes_pcm_through_the_encoder() {
        // `cat` stands in for ffmpeg: it proves the stdin→stdout plumbing,
        // including the pump thread, without needing an encoder installed.
        let mut output = Vec::new();
        encode_to_writer(
            Path::new("cat"),
            &[],
            std::io::Cursor::new(vec![7_u8; 64]),
            &mut output,
        )
        .expect("encode through cat");
        assert_eq!(output, vec![7_u8; 64]);
    }

    #[test]
    #[cfg(unix)]
    fn a_failing_encoder_reports_its_stderr() {
        let error = encode_to_writer(
            Path::new("sh"),
            &["-c".to_owned(), "echo boom >&2; exit 3".to_owned()],
            std::io::Cursor::new(Vec::new()),
            &mut Vec::new(),
        )
        .expect_err("encoder failed");
        assert!(error.contains("boom"), "stderr should reach the caller: {error}");
    }
}
