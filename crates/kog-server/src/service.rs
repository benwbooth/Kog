//! Streaming service: turn a library entry into an encoded audio stream.
//!
//! Cache-first. A cached encode is served as a plain file with `Range`
//! support, so seeking and replays are instant. On a miss the track is decoded
//! and encoded on the fly, and the bytes are tee'd into the cache as they
//! stream to the client, so the next listener gets the fast path.
//!
//! This is what makes "faster than realtime" honest: the encoder is only part
//! of the story, because Kog's emulator-backed decoders (SC-55, vgmstream,
//! GME, PSF) can render slower than realtime. Encoding once into a per-track
//! file removes that cost from every later play or seek.

use std::io::Write;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use axum::body::Bytes;
use kog_audio::decoder::{DecoderRegistry, DecoderSettings, PlaybackSource, StreamProperties};
use kog_audio::playlist::PlaylistEntry;
use kog_audio::streaming::{PcmReader, resolve_entry};

use crate::stream::{StreamCache, StreamKey, encode_to_writer};
use crate::StreamCodec;

/// Maximum encoded chunks waiting for the HTTP body to consume them.
const CHANNEL_DEPTH: usize = 16;

/// Where a stream's bytes come from.
pub enum StreamSource {
    /// A finished encode on disk: serve with `Range` support.
    Cached(PathBuf),
    /// An encode in progress: bytes arrive as they are produced.
    Encoding {
        receiver: tokio::sync::mpsc::Receiver<Result<Bytes, std::io::Error>>,
    },
    /// Part of a finished AAC encode, from the frame where a seek lands.
    CachedFrom { path: PathBuf, offset: u64 },
}

#[derive(Clone)]
pub struct StreamService {
    cache: StreamCache,
    decoder_settings: DecoderSettings,
    scratch: PathBuf,
    /// Bounds concurrent metadata expansion and native decoder probes.
    probe_lock: Arc<Mutex<()>>,
    inspection_active: Arc<Mutex<HashMap<String, PathBuf>>>,
    mml: crate::mml::MmlJobs,
    /// Songs whose level timelines are being worked out right now.
    levels_active: Arc<Mutex<HashSet<String>>>,
    /// Encodes in progress, by stream stem. A second request for the same
    /// stream (Safari's range probe, the player's prefetch of a song that
    /// then starts) reads the encode already running instead of starting
    /// another decode of the song.
    active_encodes: Arc<Mutex<HashMap<String, Arc<ActiveEncode>>>>,
}

/// One encode in progress, as the requests that follow it see it.
struct ActiveEncode {
    path: PathBuf,
    progress: Mutex<EncodeProgress>,
    changed: Condvar,
    /// Clients still reading it: the request that started it plus any that
    /// followed. An SC-55 encode nobody is listening to gives way to the
    /// next song (see `Sc55Turn`).
    listeners: AtomicUsize,
    /// Set when another song needs the SC-55: the encode stops uncached.
    cancelled: AtomicBool,
}

#[derive(Default)]
struct EncodeProgress {
    written: u64,
    done: bool,
    error: Option<String>,
}

impl ActiveEncode {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            progress: Mutex::default(),
            changed: Condvar::new(),
            listeners: AtomicUsize::new(1),
            cancelled: AtomicBool::new(false),
        }
    }

    fn wrote(&self, count: usize) {
        self.progress.lock().unwrap_or_else(|e| e.into_inner()).written += count as u64;
        self.changed.notify_all();
    }

    fn finish(&self, error: Option<String>) {
        let mut progress = self.progress.lock().unwrap_or_else(|e| e.into_inner());
        progress.done = true;
        if error.is_some() {
            progress.error = error;
        }
        drop(progress);
        self.changed.notify_all();
    }

    fn listener_left(&self) {
        let _ = self.listeners.fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| count.checked_sub(1));
    }
}

/// The server's turn at the Nuked SC-55. The emulator is shared by the whole
/// process and a new render cancels the one in progress, so two SC-55 songs
/// encoding at once (the playing song and the next one the player prefetches)
/// would cut the first off a few seconds in. SC-55 encodes take turns
/// instead. A waiting song only takes over early when nobody is listening to
/// the one rendering (the listener skipped it); that encode then stops and is
/// not cached.
struct Sc55Turn;

struct Sc55Gate {
    holder: Mutex<Option<Arc<ActiveEncode>>>,
    released: Condvar,
}

fn sc55_gate() -> &'static Sc55Gate {
    static GATE: OnceLock<Sc55Gate> = OnceLock::new();
    GATE.get_or_init(|| Sc55Gate { holder: Mutex::new(None), released: Condvar::new() })
}

impl Sc55Turn {
    fn take(active: &Arc<ActiveEncode>) -> Self {
        let gate = sc55_gate();
        let mut holder = gate.holder.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            match holder.as_ref() {
                None => {
                    *holder = Some(active.clone());
                    return Self;
                }
                Some(current) => {
                    if current.listeners.load(Ordering::Acquire) == 0 {
                        current.cancelled.store(true, Ordering::Release);
                    }
                    holder = gate
                        .released
                        .wait_timeout(holder, std::time::Duration::from_millis(250))
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
            }
        }
    }
}

impl Drop for Sc55Turn {
    fn drop(&mut self) {
        let gate = sc55_gate();
        *gate.holder.lock().unwrap_or_else(|e| e.into_inner()) = None;
        gate.released.notify_all();
    }
}

impl StreamService {
    pub fn new(
        cache: StreamCache,
        decoder_settings: DecoderSettings,
        scratch: PathBuf,
    ) -> Self {
        // The scratch playlist the decoder expansion needs lives here; create
        // it up front, or the very first stream fails on a missing directory
        // and the client sees an empty 200 instead of audio.
        let _ = std::fs::create_dir_all(&scratch);
        Self {
            cache,
            decoder_settings,
            scratch,
            probe_lock: Arc::new(Mutex::new(())),
            inspection_active: Arc::new(Mutex::new(HashMap::new())),
            mml: crate::mml::MmlJobs::default(),
            levels_active: Arc::default(),
            active_encodes: Arc::default(),
        }
    }

    pub fn cache(&self) -> &StreamCache {
        &self.cache
    }

    /// Open a stream for one entry, encoding on a miss.
    ///
    /// Everything that can fail is done here, before any bytes are promised to
    /// the client: a cache hit or an entry that will not
    /// resolve all have to become an HTTP error rather than a silent empty
    /// body.
    pub fn open(
        &self,
        entry: PlaylistEntry,
        key: StreamKey,
    ) -> Result<StreamSource, String> {
        if let Some(path) = self.cache.lookup(&key) {
            return Ok(StreamSource::Cached(path));
        }
        // A seek into a song that has been encoded once before reads the
        // finished encode from the frame it lands on. Starting a new encode
        // there means decoding up to that point first, which for emulated
        // formats can take as long as playing it.
        if key.start_ms > 0 && key.codec == crate::StreamCodec::Aac {
            if let Some(path) = self.cache.lookup(&key.clone().with_start_ms(0)) {
                if let Some(offset) = adts_offset(&path, key.start_ms) {
                    return Ok(StreamSource::CachedFrom { path, offset });
                }
            }
        }
        if let Some(following) = self.follow_active(&key) {
            return Ok(following);
        }
        let decoders = kog_audio::decoder::DecoderRegistry::new(self.decoder_settings.clone());
        let source = resolve_entry(&entry, &decoders, &self.scratch)?;
        // The resolved source of an archive member lives in the registry's
        // extraction workspace, which is deleted when the registry drops:
        // keep it alive for the whole encode.
        self.start_encode(source, key, decoders)
    }

    /// The track's MML score, recorded with the same synth as its stream.
    pub fn mml(
        &self,
        entry: PlaylistEntry,
        key: &StreamKey,
        title: String,
        have: Option<u64>,
        bars: usize,
    ) -> crate::mml::Status {
        // A second decode alongside the stream: see `silent_render_settings`.
        let settings = self.silent_render_settings(key);
        let job_key = format!("{}\0{}\0{}", key.locator, key.render_profile.as_deref().unwrap_or_default(), key.looping);
        self.mml.status(job_key, title, entry, settings, self.scratch.join("mml"), have, bars)
    }

    /// Settings for a silent decode that runs beside the song's stream (its
    /// levels, its MML score). The Nuked SC-55 is one shared emulator per ROM
    /// set and a new render cancels the one in progress, so a second SC-55
    /// render would cut the stream off a few seconds in; these use the OPL3
    /// synth instead, which plays the same notes.
    fn silent_render_settings(&self, key: &StreamKey) -> DecoderSettings {
        self.render_settings(key).for_recording()
    }

    fn render_settings(&self, key: &StreamKey) -> DecoderSettings {
        let engine = key.render_profile.as_deref().and_then(kog_audio::settings::MidiEngine::from_setting)
            .unwrap_or_else(|| self.decoder_settings.midi_engine());
        DecoderSettings::new(self.decoder_settings.soundfont_path(), engine)
            .with_sc55_rom_path(self.decoder_settings.sc55_rom_path())
            .with_mt32_rom_path(self.decoder_settings.mt32_rom_path())
            .with_mt32_gm_program_mapping(self.decoder_settings.mt32_gm_program_mapping())
    }

    pub fn channel_window(&self, key: &StreamKey, position: f64) -> Result<Option<kog_inspection::Window>, String> {
        let active = self.inspection_active.lock().unwrap_or_else(|e| e.into_inner()).get(&key.stem()).cloned();
        if let Some(path) = active {
            if let Some(window) = crate::inspection::read_window(&path, position)? { return Ok(Some(window)); }
        }
        crate::inspection::read_window(&self.cache.inspection_path(key), position)
    }

    /// The song's level timeline so far (six bytes per meter update) and
    /// whether it is complete. The first request starts working it out; it is
    /// its own decode, so it needs nothing from the stream's encode or cache.
    pub fn levels(&self, entry: PlaylistEntry, key: &StreamKey) -> Result<(Vec<u8>, bool), String> {
        let directory = self.cache.levels_directory();
        let stem = key.stem();
        let finished = directory.join(format!("{stem}.levels"));
        if let Ok(bytes) = std::fs::read(&finished) {
            return Ok((bytes, true));
        }
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let partial = directory.join(format!("{stem}.levels.part"));
        if self.levels_active.lock().unwrap_or_else(|e| e.into_inner()).insert(stem.clone()) {
            let service = self.clone();
            let key = key.clone();
            let path = partial.clone();
            std::thread::spawn(move || {
                let result = service.write_levels(entry, &key, &path)
                    .and_then(|()| std::fs::rename(&path, &finished).map_err(|error| error.to_string()));
                if let Err(error) = result {
                    eprintln!("kog-server: working out song levels: {error}");
                    let _ = std::fs::remove_file(&path);
                }
                service.levels_active.lock().unwrap_or_else(|e| e.into_inner()).remove(&stem);
            });
        }
        Ok((std::fs::read(&partial).unwrap_or_default(), false))
    }

    fn write_levels(&self, entry: PlaylistEntry, key: &StreamKey, path: &Path) -> Result<(), String> {
        let settings = self.silent_render_settings(key);
        let decoders = DecoderRegistry::new(settings.clone());
        // Archive members live in the registry's workspace, kept to the end.
        let source = resolve_entry(&entry, &decoders, &self.scratch)?;
        let mut pcm = PcmReader::open(source, settings)?;
        let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
        kog_audio::streaming::write_levels(&mut pcm, &mut std::io::BufWriter::new(file))
    }

    /// Read one entry's tags without encoding it, using the same resolution
    /// path streaming uses, so archive members, cue fragments and subsongs are
    /// addressed identically. Probing is blocking; callers run it on a
    /// blocking thread.
    ///
    /// Serialized to bound native probe work. Each entry resolves in a unique
    /// temporary directory so metadata and playback requests cannot mix.
    pub fn probe_entry(&self, entry: PlaylistEntry) -> Result<StreamProperties, String> {
        self.probe_entry_with_size(entry).map(|(properties, ..)| properties)
    }

    /// Probe tags and capture the resolved file's size during the same lookup.
    /// Remote URLs have no local file size; archive members report their
    /// extracted member size rather than the outer archive's size.
    pub fn probe_entry_with_size(
        &self,
        entry: PlaylistEntry,
    ) -> Result<(StreamProperties, Option<u64>, Option<&'static str>), String> {
        let _guard = self
            .probe_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let decoders = DecoderRegistry::new(self.decoder_settings.clone());
        let scratch = self.scratch.join("metadata");
        let source = resolve_entry(&entry, &decoders, &scratch)?;
        let file_size_bytes = if source.is_remote() {
            None
        } else {
            std::fs::metadata(&source.path).ok().map(|metadata| metadata.len())
        };
        let backend = decoders.selected_backend_name(&source);
        let mut properties = decoders.probe(&source)?;
        // The album artist and composer never come from a decoder backend;
        // read them the way the desktop's tag path does.
        if !source.is_remote() {
            properties.fill_local_tags(&source.path);
            let no_main_tags = properties.title.is_none()
                && properties.artist.is_none()
                && properties.album.is_none();
            if (no_main_tags || kog_audio::legacy_id3::looks_misdecoded(&[
                properties.title.as_deref().unwrap_or_default(),
                properties.artist.as_deref().unwrap_or_default(),
                properties.album.as_deref().unwrap_or_default(),
                properties.album_artist.as_deref().unwrap_or_default(),
                properties.composer.as_deref().unwrap_or_default(),
                properties.genre.as_deref().unwrap_or_default(),
            ])) && let Some(corrected) = kog_audio::legacy_id3::corrected_text(&source.path) {
                if let Some(value) = corrected.title {
                    properties.title = Some(value);
                }
                if let Some(value) = corrected.artist {
                    properties.artist = Some(value);
                }
                if let Some(value) = corrected.album {
                    properties.album = Some(value);
                }
                if let Some(value) = corrected.album_artist {
                    properties.album_artist = Some(value);
                }
                if let Some(value) = corrected.composer {
                    properties.composer = Some(value);
                }
                if let Some(value) = corrected.genre {
                    properties.genre = Some(value);
                }
            }
        }
        // As on the desktop, a placeholder title (a MIDI file's "A", "Seq-1")
        // gives way to the cleaned-up filename, and a short one gains it.
        if let Some(title) = kog_audio::track::better_title(
            properties.title.as_deref().unwrap_or_default(),
            &source,
        ) {
            properties.title = Some(title);
        }
        // As on the desktop, a song without an album takes its folder's or
        // archive's name.
        if properties.album.as_deref().is_none_or(|album| album.trim().is_empty()) {
            if let Some(album) = kog_audio::track::album_from_folder(&source) {
                properties.album = Some(album);
            }
        }
        Ok((properties, file_size_bytes, backend))
    }

    /// The synthesizer new MIDI streams decode with. `DecoderSettings`
    /// shares its options through Arc'd locks, so setting this retunes every
    /// holder, including the clones the streaming path already made.
    pub fn midi_engine(&self) -> kog_audio::settings::MidiEngine {
        self.decoder_settings.midi_engine()
    }

    /// Retune the running service: new streams decode with the chosen
    /// synthesizer, and the choice is persisted for the desktop's next start
    /// by the caller.
    pub fn set_midi_engine(&self, engine: kog_audio::settings::MidiEngine) {
        self.decoder_settings.set_midi_engine(engine);
    }

    /// Follow the encode of this stream already in progress, if there is one:
    /// its bytes so far from the cache file, then the rest as they arrive.
    fn follow_active(&self, key: &StreamKey) -> Option<StreamSource> {
        let active = self.active_encodes.lock().unwrap_or_else(|e| e.into_inner());
        let encode = active.get(&key.stem())?.clone();
        // Opened while the encode is still registered, so before it renames
        // its file into the cache; the open handle survives the rename.
        let file = std::fs::File::open(&encode.path).ok()?;
        encode.listeners.fetch_add(1, Ordering::AcqRel);
        drop(active);
        let (sender, receiver) = tokio::sync::mpsc::channel(CHANNEL_DEPTH);
        let spawned = std::thread::Builder::new()
            .name("kog-stream-follow".to_owned())
            .spawn({
                let encode = encode.clone();
                move || {
                    follow_encode(&encode, file, &sender);
                    encode.listener_left();
                }
            });
        if spawned.is_err() {
            encode.listener_left();
            return None;
        }
        Some(StreamSource::Encoding { receiver })
    }

    /// Stop offering an encode to new requests and wake the ones following
    /// it. Called before a finished encode is moved into the cache, so a
    /// follower never opens a file that has just been renamed.
    fn retire_encode(&self, key: &StreamKey, encode: &Arc<ActiveEncode>, error: Option<String>) {
        let mut active = self.active_encodes.lock().unwrap_or_else(|e| e.into_inner());
        if active.get(&key.stem()).is_some_and(|current| Arc::ptr_eq(current, encode)) {
            active.remove(&key.stem());
        }
        drop(active);
        encode.finish(error);
    }

    fn start_encode(
        &self,
        source: PlaybackSource,
        key: StreamKey,
        decoders: kog_audio::decoder::DecoderRegistry,
    ) -> Result<StreamSource, String> {
        let (sender, receiver) = tokio::sync::mpsc::channel(CHANNEL_DEPTH);
        let (partial_path, partial) = self.cache.create_unique_partial(&key)?;
        let encode = {
            let mut active = self.active_encodes.lock().unwrap_or_else(|e| e.into_inner());
            if active.contains_key(&key.stem()) {
                // Another request started this stream meanwhile: follow it.
                drop(active);
                drop(partial);
                let _ = std::fs::remove_file(&partial_path);
                if let Some(following) = self.follow_active(&key) {
                    return Ok(following);
                }
                return Err("the stream's encode ended while starting".to_owned());
            }
            let encode = Arc::new(ActiveEncode::new(partial_path.clone()));
            active.insert(key.stem(), encode.clone());
            encode
        };
        let service = self.clone();
        let encode_handle = encode.clone();
        let partial_path_handle = partial_path.clone();
        let key_handle = key.clone();
        // Decoding and encoding are blocking, CPU-bound work: keep them off
        // the async runtime's worker threads.
        let spawned = std::thread::Builder::new()
            .name("kog-stream-encode".to_owned())
            .spawn(move || {
                // Holding the registry here keeps any archive extraction
                // workspace alive for as long as the source needs it.
                let _decoders = decoders;
                let result = service.encode_into(source, &key, partial, &partial_path, sender.clone(), &encode);
                if let Err(error) = &result {
                    let _ = std::fs::remove_file(&partial_path);
                    eprintln!("kog-server: streaming {}: {error}", key.locator);
                    service.retire_encode(&key, &encode, Some(error.clone()));
                    let _ = sender.blocking_send(Err(std::io::Error::other(error.clone())));
                }
            });
        if let Err(error) = spawned {
            self.retire_encode(&key_handle, &encode_handle, Some(error.to_string()));
            let _ = std::fs::remove_file(&partial_path_handle);
            return Err(format!("starting the stream encoder: {error}"));
        }
        Ok(StreamSource::Encoding { receiver })
    }

    fn encode_into(
        &self,
        source: PlaybackSource,
        key: &StreamKey,
        partial: std::fs::File,
        partial_path: &Path,
        sender: tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
        encode: &Arc<ActiveEncode>,
    ) -> Result<(), String> {
        // Freeze the render profile that named this stream and its channel
        // recording, even if the user changes synths during an encode.
        let settings = self.render_settings(key);
        // Declared before the decoder so it is released after it.
        let sc55_turn = (key.render_profile.as_deref()
            == Some(kog_audio::settings::MidiEngine::Sc55.setting_value()))
        .then(|| Sc55Turn::take(encode));
        let sc55_failures = kog_audio::decoder::sc55_render_failures();
        let mut pcm = PcmReader::open(source, settings)?.with_leveling(key.leveling);
        if key.start_ms > 0 {
            pcm.seek(std::time::Duration::from_millis(key.start_ms))?;
        }
        let metadata_path = partial_path.with_extension("channels.part");
        let metadata = std::fs::File::create(&metadata_path).map_err(|e| e.to_string())?;
        self.inspection_active.lock().unwrap_or_else(|e| e.into_inner()).insert(key.stem(), metadata_path.clone());
        let active_guard = InspectionActive {
            active: self.inspection_active.clone(), key: key.stem(), path: metadata_path.clone(),
        };
        let sample_rate = pcm.sample_rate();
        let channels = pcm.channels();
        let tee = TeeWriter {
            file: partial,
            sender,
            encode: Some(encode.clone()),
            client_gone: false,
        };
        let pcm = crate::inspection::CaptureReader::new(pcm, metadata, key.start_ms)?;
        encode_to_writer(key.codec, key.bitrate_kbps, sample_rate, channels, pcm, tee)?;
        // A cut-short SC-55 render ends its song early and the rest is
        // silence: don't keep it as the song.
        if sc55_turn.is_some() && kog_audio::decoder::sc55_render_failures() != sc55_failures {
            return Err("the Nuked SC-55 render stopped before the song ended".to_owned());
        }
        drop(sc55_turn);
        self.retire_encode(key, encode, None);
        let final_metadata = self.cache.inspection_path(key);
        if !final_metadata.exists() {
            if let Err(error) = std::fs::rename(&metadata_path, &final_metadata) {
                eprintln!("kog-server: saving channel metadata: {error}");
            }
        }
        if let Err(error) = self.cache.commit(key, partial_path) {
            if self.cache.lookup(key).is_none() {
                let _ = std::fs::remove_file(&final_metadata);
            }
            return Err(error);
        }
        drop(active_guard);
        Ok(())
    }
}

struct InspectionActive {
    active: Arc<Mutex<HashMap<String, PathBuf>>>,
    key: String,
    path: PathBuf,
}
impl Drop for InspectionActive {
    fn drop(&mut self) {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.get(&self.key) == Some(&self.path) { active.remove(&self.key); }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Writes encoded bytes to the cache file and to the client at once.
///
/// A client that disconnects (or a filled channel) does not abort the encode:
/// finishing the cache entry is worth more than the abandoned response, since
/// every later play of that track then takes the cached path.
/// The byte offset of the ADTS frame `start_ms` into an AAC file, or None
/// when the file is not ADTS or ends first. Each frame holds 1024 samples.
fn adts_offset(path: &Path, start_ms: u64) -> Option<u64> {
    const RATES: [u64; 13] = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];
    let data = std::fs::read(path).ok()?;
    let mut offset = 0usize;
    let mut samples = 0u64;
    loop {
        let header = data.get(offset..offset + 7)?;
        if header[0] != 0xff || header[1] & 0xf0 != 0xf0 {
            return None;
        }
        let rate = *RATES.get(usize::from((header[2] >> 2) & 0x0f))?;
        if samples * 1000 >= start_ms * rate {
            return Some(offset as u64);
        }
        let length = (usize::from(header[3] & 0x03) << 11) | (usize::from(header[4]) << 3) | usize::from(header[5] >> 5);
        if length < 7 {
            return None;
        }
        offset += length;
        samples += 1024 * (u64::from(header[6] & 0x03) + 1);
    }
}

struct TeeWriter {
    file: std::fs::File,
    sender: tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
    /// The encode other requests follow, told about every write.
    encode: Option<Arc<ActiveEncode>>,
    client_gone: bool,
}

impl Write for TeeWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if self.encode.as_ref().is_some_and(|encode| encode.cancelled.load(Ordering::Acquire)) {
            return Err(std::io::Error::other("stopped: nobody was listening and another song needed the SC-55"));
        }
        self.file.write_all(buffer)?;
        if let Some(encode) = &self.encode {
            encode.wrote(buffer.len());
        }
        if !self.client_gone
            && self.sender.blocking_send(Ok(Bytes::copy_from_slice(buffer))).is_err()
        {
            self.client_gone = true;
            if let Some(encode) = &self.encode {
                encode.listener_left();
            }
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

/// Send a follower everything the encode has written, waiting for more until
/// it finishes.
fn follow_encode(
    encode: &ActiveEncode,
    mut file: std::fs::File,
    sender: &tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
) {
    use std::io::Read;
    let mut sent = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let (written, done, error) = {
            let mut progress = encode.progress.lock().unwrap_or_else(|e| e.into_inner());
            while progress.written <= sent && !progress.done {
                progress = encode.changed.wait(progress).unwrap_or_else(|e| e.into_inner());
            }
            (progress.written, progress.done, progress.error.clone())
        };
        while sent < written {
            let want = usize::try_from(written - sent).unwrap_or(usize::MAX).min(buffer.len());
            match file.read(&mut buffer[..want]) {
                Ok(0) => {
                    // Written but not readable yet; look again shortly.
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    break;
                }
                Ok(count) => {
                    sent += count as u64;
                    if sender.blocking_send(Ok(Bytes::copy_from_slice(&buffer[..count]))).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = sender.blocking_send(Err(error));
                    return;
                }
            }
        }
        if done {
            if let Some(error) = error {
                let _ = sender.blocking_send(Err(std::io::Error::other(error)));
            }
            return;
        }
    }
}

/// Locate the cache root: `KOG_CACHE_DIR` when set (tests), else Kog's cache
/// directory beside cover art.
pub fn cache_root() -> PathBuf {
    if let Some(directory) = std::env::var_os("KOG_CACHE_DIR") {
        return PathBuf::from(directory).join("streams");
    }
    kog_audio::settings::setting_path("cache")
        .map(|path| path.join("streams"))
        .unwrap_or_else(|| PathBuf::from("/tmp/kog-streams"))
}

/// Scratch directory for the temporary one-line playlists used to resolve
/// entries. Kept beside the cache so it inherits the same lifetime and cleanup.
pub fn scratch_root() -> PathBuf {
    cache_root().join("scratch")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::StreamCache;
    use kog_audio::decoder::{DecoderRegistry, PlaybackSource};
    use kog_audio::playlist::PlaylistLocation;
    use kog_audio::track::Track;

    fn cache() -> (tempfile::TempDir, StreamCache) {
        let directory = tempfile::tempdir().unwrap();
        let cache = StreamCache::new(directory.path().join("streams"), 1 << 20);
        (directory, cache)
    }

    #[test]
    fn a_cached_encode_is_served_without_re_encoding() {
        let (_dir, cache) = cache();
        let key = StreamKey::new("/music/song.flac", StreamCodec::Aac, 192);
        let entry_path = cache.entry_path(&key);
        std::fs::create_dir_all(entry_path.parent().unwrap()).unwrap();
        std::fs::write(&entry_path, b"cached bytes").unwrap();

        let service = StreamService::new(
            cache,
            DecoderSettings::default(),
            PathBuf::from("/tmp"),
        );
        let entry = PlaylistEntry {
            location: PlaylistLocation::Local(PathBuf::from("/music/song.flac")),
            fragment: None,
        };
        match service.open(entry, key).expect("cached stream") {
            StreamSource::Cached(path) => assert_eq!(path, entry_path),
            StreamSource::Encoding { .. } => panic!("a cached entry must not re-encode"),
            StreamSource::CachedFrom { .. } => panic!("a cached entry is served whole"),
        }
    }

    #[test]
    fn levels_cover_the_whole_song_for_clients_that_cannot_tap_their_audio() {
        let (directory, cache) = cache();
        let path = directory.path().join("tone.wav");
        let samples: Vec<u8> = (0..44_100)
            .flat_map(|index| {
                let phase = index as f32 * 440.0 * std::f32::consts::TAU / 44_100.0;
                ((phase.sin() * 20_000.0) as i16).to_le_bytes()
            })
            .collect();
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        wav.extend_from_slice(&44_100_u32.to_le_bytes());
        wav.extend_from_slice(&88_200_u32.to_le_bytes());
        wav.extend_from_slice(b"\x02\0\x10\0data");
        wav.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        wav.extend_from_slice(&samples);
        std::fs::write(&path, wav).unwrap();

        let scratch = directory.path().join("scratch");
        let service = StreamService::new(cache, DecoderSettings::default(), scratch);
        let key = StreamKey::new(path.to_string_lossy(), StreamCodec::Aac, 192);
        let entry = PlaylistEntry { location: PlaylistLocation::Local(path), fragment: None };
        let levels = (0..600)
            .find_map(|_| {
                let (bytes, complete) = service.levels(entry.clone(), &key).expect("levels");
                if !complete {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                complete.then_some(bytes)
            })
            .expect("levels finish");
        assert_eq!(levels.len() % 6, 0);
        assert!((18..=22).contains(&(levels.len() / 6)), "{} ticks", levels.len() / 6);
        assert!(levels.iter().any(|level| *level > 0));
    }

    #[test]
    fn inspection_metadata_uses_the_encoded_audio_clock_and_survives_a_seek() {
        use kog_audio::settings::MidiEngine;
        let (directory, cache) = cache();
        let path = directory.path().join("inspect.mid");
        // C4 for one second, then E4 for one second, at 60 BPM.
        let track = [0,0xff,0x51,3,0x0f,0x42,0x40,0,0x90,60,100,
            0x83,0x60,0x80,60,0,0,0x90,64,100,0x83,0x60,0x80,64,0,0,0xff,0x2f,0];
        let mut midi = b"MThd\0\0\0\x06\0\0\0\x01\x01\xe0MTrk".to_vec();
        midi.extend_from_slice(&(track.len() as u32).to_be_bytes());midi.extend_from_slice(&track);
        std::fs::write(&path,midi).unwrap();
        let settings=DecoderSettings::new(None,MidiEngine::Opl3Windows);
        let service=StreamService::new(cache.clone(),settings,directory.path().join("scratch"));
        for start_ms in [0,1200] {
            let key=StreamKey::new(path.to_string_lossy(),StreamCodec::Flac,192)
                .with_render_profile(MidiEngine::Opl3Windows.setting_value()).with_start_ms(start_ms);
            let opened=service.open(PlaylistEntry{location:PlaylistLocation::Local(path.clone()),fragment:None},key.clone()).unwrap();
            let StreamSource::Encoding {mut receiver}=opened else {panic!("expected encode")};
            let mut bytes=0;
            while let Some(chunk)=receiver.blocking_recv(){bytes+=chunk.unwrap().len();}
            assert!(bytes>100);
            let time=if start_ms==0 {0.2} else {1.3};
            let window=service.channel_window(&key,time).unwrap().expect("recorded window");
            let snapshot=window.snapshot(time,false,false);
            assert_eq!(snapshot.channels[0].notes[0].key,if start_ms==0 {60.0} else {64.0});
            assert!(!snapshot.playing);
            assert!(cache.inspection_path(&key).is_file());
            if let Some(destination)=std::env::var_os("KOG_INSPECTION_FIXTURES") {
                let destination=Path::new(&destination);std::fs::create_dir_all(destination).unwrap();
                std::fs::copy(&path,destination.join("fixture.mid")).unwrap();
                std::fs::write(destination.join(format!("window-{start_ms}.json")),serde_json::to_vec_pretty(&window).unwrap()).unwrap();
            }
        }
        cache.clear();assert!(!cache.root().exists());
    }

    #[test]
    fn uncached_seek_encodes_from_the_requested_audio_position() {
        use std::io::Read;
        let (directory, cache) = cache();
        // Two seconds: silence followed by a tone. A seek must return the
        // tone immediately and only the remaining half-second of audio.
        let path = directory.path().join("seek.wav");
        let frames = 16_000_u32;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + frames * 2).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&16_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(frames * 2).to_le_bytes());
        for frame in 0..frames {
            let sample = if frame < 8_000 { 0 } else {
                ((f64::from(frame) * std::f64::consts::TAU * 440.0 / 8_000.0).sin() * 8192.0) as i16
            };
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(&path, wav).unwrap();
        let full_key = StreamKey::new(path.to_string_lossy(), StreamCodec::Flac, 192);
        let key = full_key.clone().with_start_ms(1_500);
        assert_ne!(key.stem(), full_key.stem());
        let service = StreamService::new(cache.clone(), DecoderSettings::default(), directory.path().join("scratch"));
        let opened = service.open(PlaylistEntry {
            location: PlaylistLocation::Local(path), fragment: None,
        }, key.clone()).unwrap();
        let StreamSource::Encoding { mut receiver } = opened else { panic!("expected an uncached encode") };
        let mut encoded = Vec::new();
        while let Some(chunk) = receiver.blocking_recv() { encoded.extend_from_slice(&chunk.unwrap()); }
        assert!(!encoded.is_empty());
        assert!(cache.lookup(&key).is_some());
        assert!(cache.lookup(&full_key).is_none());
        let output = directory.path().join("result.flac");
        std::fs::write(&output, encoded).unwrap();
        let mut pcm = PcmReader::open_stream(output.to_str().unwrap(), "", None).unwrap();
        let mut samples = Vec::new();
        (&mut pcm).take(48_000 * 8 * 2).read_to_end(&mut samples).unwrap();
        assert!(samples.len() >= 48_000 * 8 * 4 / 10, "missing audio after seek");
        assert!(samples.len() < 48_000 * 8 * 6 / 10, "seek streamed the whole track");
        assert!(samples[..4096].chunks_exact(4).any(|s| f32::from_le_bytes(s.try_into().unwrap()).abs() > 0.05), "seek began at the initial silence");
    }

    #[test]
    fn the_tee_writer_mirrors_bytes_and_survives_a_dead_client() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("out.bin");
        let (sender, mut receiver) = tokio::sync::mpsc::channel(2);
        let mut tee = TeeWriter {
            file: std::fs::File::create(&path).unwrap(),
            sender,
            encode: None,
            client_gone: false,
        };
        tee.write_all(b"first").unwrap();
        drop(receiver); // client went away
        // A disconnected client must not fail the encode: the cache matters.
        tee.write_all(b"second").unwrap();
        tee.flush().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"firstsecond");

        // And with a live client the bytes are mirrored.
        let (sender, mut receiver) = tokio::sync::mpsc::channel(2);
        let mut tee = TeeWriter {
            file: std::fs::File::create(directory.path().join("out2.bin")).unwrap(),
            sender,
            encode: None,
            client_gone: false,
        };
        tee.write_all(b"streamed").unwrap();
        let received = receiver.try_recv().unwrap().unwrap();
        assert_eq!(received.as_ref(), b"streamed");
    }

    #[test]
    fn local_probe_reports_file_bytes_without_using_stream_size() {
        let (directory, cache) = cache();
        let path = directory.path().join("song.wav");
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&36_u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&16_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&0_u32.to_le_bytes());
        std::fs::write(&path, &wav).unwrap();
        let desktop = Track::from_source(
            PlaybackSource::from_path(path.clone()),
            &DecoderRegistry::default(),
        );
        assert_eq!(desktop.file_size_bytes, Some(wav.len() as u64));
        let service = StreamService::new(
            cache,
            DecoderSettings::default(),
            directory.path().join("scratch"),
        );
        let (_, file_size_bytes, _) = service
            .probe_entry_with_size(PlaylistEntry {
                location: PlaylistLocation::Local(path),
                fragment: None,
            })
            .unwrap();
        assert_eq!(file_size_bytes, Some(wav.len() as u64));
    }

    #[test]
    #[ignore = "requires KOG_TEST_LEGACY_MP3 pointing to an MP3 with mislabelled ID3 text"]
    fn legacy_mp3_metadata_matches_in_desktop_and_web_probe() {
        let path = PathBuf::from(std::env::var_os("KOG_TEST_LEGACY_MP3").expect("fixture path"));
        let decoders = DecoderRegistry::default();
        let track = Track::from_source(PlaybackSource::from_path(path.clone()), &decoders);
        assert_eq!(track.title, "I Believe");
        assert_eq!(track.artist, "孙楠");
        assert_eq!(track.album, "缘份的天空");

        let (directory, cache) = cache();
        let service = StreamService::new(
            cache,
            DecoderSettings::default(),
            directory.path().join("scratch"),
        );
        let properties = service
            .probe_entry(PlaylistEntry {
                location: PlaylistLocation::Local(path),
                fragment: None,
            })
            .unwrap();
        assert_eq!(properties.title.as_deref(), Some("I Believe"));
        assert_eq!(properties.artist.as_deref(), Some("孙楠"));
        assert_eq!(properties.album.as_deref(), Some("缘份的天空"));
    }
}
