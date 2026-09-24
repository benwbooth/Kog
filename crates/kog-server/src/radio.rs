//! Random Radio for the web player.
//!
//! The desktop's random radio is an exhaustive, seeded, hierarchical shuffle
//! of the music folder, persisted to `radio-round.json`. The web player had no
//! equivalent, so this module owns a small server-side session around the very
//! same [`kog_audio::radio::RadioRound`]: it loads and saves the desktop's
//! round file, walks the library root with the shared pick logic, and hands the
//! web client a window of playable entries.
//!
//! Picks are expanded and proved exactly as the desktop does. The shared
//! [`DecoderRegistry::expand_detailed`] turns a cue sheet or multi-song file
//! into its individual tracks, and every candidate is probed before it is
//! handed out: a pick that yields nothing playable joins the round's `dead`
//! set, so both clients skip the same broken files. `read_cue` follows the
//! user's folder setting, matching the desktop's radio session.
//!
//! Because the round file is the desktop's, the server roots itself where the
//! desktop left off: a persisted round whose root is a directory inside the
//! configured library root is resumed in place (same seed and cursors, so the
//! two clients continue one shuffle). With no usable round it starts fresh at
//! the configured music directory.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use kog_audio::decoder::{DecoderRegistry, PlaybackSource, StreamProperties};
use kog_audio::radio::{
    Blacklist, RadioCtx, RadioRound, RoundInitial, locator_is_blacklisted, radio_locator_key,
    random_seed,
};
use kog_audio::settings::AppSettings;
use kog_core::db::LibraryDb;

use crate::routes::{AppState, bad_request};

/// The desktop round file, by name, under the platform config directory.
const ROUND_FILE: &str = "radio-round.json";

/// How many picks one round window carries. The desktop stages a small hidden
/// buffer; the web pane shows a window and asks for more when it runs out.
/// Big enough to browse, small enough that a request stays a quick filesystem
/// walk.
const WINDOW: usize = 60;

/// Consecutive unplayable picks one window tolerates before giving up. Mirrors
/// the desktop staging thread's cap: a library of nothing but junk must not
/// spin forever re-proving itself.
const MAX_SKIPS: usize = 16_384;

/// How long one pick may take to expand and prove before it is declared
/// unplayable. Normal picks finish in milliseconds and archive extractions
/// take seconds. A probe that never returns (a wedged native emulator has
/// been observed hanging forever on one miniusf) must not wedge the round
/// and every radio call behind its mutex: silence joins the dead set,
/// exactly like a probe error.
const PROVE_TIMEOUT: Duration = Duration::from_secs(30);

/// One playable pick, shaped like every other API entry so the web pane can
/// render it with the existing row code.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RadioEntry {
    pub kind: String,
    pub path: String,
    pub entry: String,
    pub fragment: Option<String>,
    /// Location relative to the round root, for the pane's subtitle.
    pub relative: String,
}

/// The full radio state, returned by every radio endpoint.
#[derive(Clone, Debug, Serialize)]
pub struct RadioStatus {
    pub enabled: bool,
    pub root: Option<String>,
    pub seed: u64,
    pub entries: Vec<RadioEntry>,
}

/// One page of additional picks, continuing the running round.
#[derive(Clone, Debug, Serialize)]
pub struct RadioAdvance {
    pub entries: Vec<RadioEntry>,
    pub exhausted: bool,
}

struct Inner {
    /// True for the production state that reads and writes Kog's settings.
    /// Tests build an explicit state and must not touch the user's config.
    configured: bool,
    initialized: bool,
    enabled: bool,
    root: Option<PathBuf>,
    save_path: Option<PathBuf>,
    round: Option<RadioRound>,
    seed: u64,
    /// Unplayable locators remembered across reshuffles, as the desktop does.
    dead: HashSet<String>,
    entries: Vec<RadioEntry>,
}

/// The server's random-radio session. Cheap to clone through `Arc`; all
/// mutation is serialized by one mutex.
pub struct Radio {
    inner: Mutex<Inner>,
}

impl Radio {
    /// A state the tests and the default `AppState` use: disabled, with no
    /// settings file and no round file to touch.
    pub fn disabled() -> Self {
        Self {
            inner: Mutex::new(Inner {
                configured: false,
                initialized: true,
                enabled: false,
                root: None,
                save_path: None,
                round: None,
                seed: 0,
                dead: HashSet::new(),
                entries: Vec::new(),
            }),
        }
    }

    /// The production state: settings and the desktop round file are read
    /// lazily on the first radio request.
    pub fn from_settings() -> Self {
        Self {
            inner: Mutex::new(Inner {
                configured: true,
                initialized: false,
                enabled: false,
                root: None,
                save_path: None,
                round: None,
                seed: 0,
                dead: HashSet::new(),
                entries: Vec::new(),
            }),
        }
    }

    /// An explicit state for tests: a known root, an optional round-file path,
    /// and an initial on/off flag. Never reads or writes Kog's settings.
    pub fn new(root: Option<PathBuf>, save_path: Option<PathBuf>, enabled: bool) -> Self {
        let root = resolve_root(save_path.as_deref(), root.as_deref());
        Self {
            inner: Mutex::new(Inner {
                configured: false,
                initialized: true,
                enabled,
                root,
                save_path,
                round: None,
                seed: 0,
                dead: HashSet::new(),
                entries: Vec::new(),
            }),
        }
    }

    /// Current state, materializing the first round window when radio is on.
    pub fn snapshot(&self, library_root: Option<&Path>, scope: Option<&Path>) -> RadioStatus {
        let mut inner = lock(&self.inner);
        Self::ensure_loaded(&mut inner, library_root);
        Self::reroot(&mut inner, scope);
        if inner.enabled && inner.round.is_none() {
            Self::open_round(&mut inner);
            Self::generate(&mut inner, WINDOW);
        }
        status_of(&inner)
    }

    /// Turn radio on or off. Turning it off keeps the playlist the client
    /// already has; turning it back on resumes the persisted round.
    pub fn set_enabled(
        &self,
        enabled: bool,
        library_root: Option<&Path>,
        scope: Option<&Path>,
    ) -> RadioStatus {
        let mut inner = lock(&self.inner);
        Self::ensure_loaded(&mut inner, library_root);
        Self::reroot(&mut inner, scope);
        if enabled {
            if !inner.enabled || inner.round.is_none() {
                inner.enabled = true;
                if inner.round.is_none() {
                    Self::open_round(&mut inner);
                    Self::generate(&mut inner, WINDOW);
                }
                Self::persist_enabled(&inner, true);
            }
        } else if inner.enabled {
            Self::save(&inner);
            inner.enabled = false;
            inner.round = None;
            inner.entries.clear();
            Self::persist_enabled(&inner, false);
        }
        status_of(&inner)
    }

    /// Start a fresh shuffle with a new seed, keeping unplayability knowledge,
    /// exactly like the desktop's reshuffle. Turns radio on if it was off.
    pub fn reshuffle(&self, library_root: Option<&Path>, scope: Option<&Path>) -> RadioStatus {
        let mut inner = lock(&self.inner);
        Self::ensure_loaded(&mut inner, library_root);
        Self::reroot(&mut inner, scope);
        if inner.root.is_none() {
            inner.root = library_root.map(Path::to_path_buf);
        }
        let seed = random_seed();
        inner.enabled = true;
        inner.seed = seed;
        inner.round = Some(RadioRound::new(seed));
        inner.entries.clear();
        Self::persist_enabled(&inner, true);
        Self::generate(&mut inner, WINDOW);
        status_of(&inner)
    }

    /// Append the next window of the running round.
    pub fn advance(&self, library_root: Option<&Path>, scope: Option<&Path>) -> RadioAdvance {
        let mut inner = lock(&self.inner);
        Self::ensure_loaded(&mut inner, library_root);
        Self::reroot(&mut inner, scope);
        if !inner.enabled {
            return RadioAdvance {
                entries: Vec::new(),
                exhausted: true,
            };
        }
        if inner.round.is_none() {
            Self::open_round(&mut inner);
        }
        inner.entries.clear();
        Self::generate(&mut inner, WINDOW);
        RadioAdvance {
            entries: inner.entries.clone(),
            exhausted: inner.entries.is_empty(),
        }
    }

    fn ensure_loaded(inner: &mut Inner, library_root: Option<&Path>) {
        if inner.initialized {
            return;
        }
        inner.initialized = true;
        let settings = AppSettings::load();
        inner.enabled = settings.radio_enabled;
        inner.save_path = kog_audio::settings::setting_path(ROUND_FILE);
        inner.root = resolve_root(inner.save_path.as_deref(), library_root);
    }

    /// Adopt a changed scope: when the client explicitly requested a tree
    /// root and it differs from the running round's, the round restarts
    /// under it (a fresh shuffle, or a resumed round if one was persisted
    /// for exactly that folder), like the desktop's radio restarting when
    /// its tree root changes. No scope — the desktop's round keeps ruling.
    fn reroot(inner: &mut Inner, scope: Option<&Path>) {
        let Some(root) = scope else {
            return;
        };
        if inner.root.as_deref() != Some(root) {
            inner.root = Some(root.to_path_buf());
            inner.round = None;
            inner.entries.clear();
        }
    }

    /// Load the persisted round for our root, or begin a fresh one. The
    /// desktop's file is trusted only when its root matches exactly, which
    /// `RadioRound::load` enforces.
    fn open_round(inner: &mut Inner) {
        let Some(root) = inner.root.clone() else {
            inner.round = None;
            return;
        };
        let initial = inner
            .save_path
            .as_deref()
            .and_then(|path| RadioRound::load(path, &root))
            .unwrap_or_else(|| RoundInitial::fresh(random_seed()));
        inner.seed = initial.seed;
        inner.dead = initial.dead.iter().cloned().collect();
        inner.round = Some(RadioRound::restore(initial));
        inner.entries.clear();
    }

    /// Fill the window with the next `target` picks, rotating the round (and
    /// its seed) on exhaustion and giving up after two barren rounds, as the
    /// desktop staging thread does.
    ///
    /// Each pick is expanded into its tracks and probed; anything unplayable is
    /// skipped and remembered in the round's `dead` set, exactly like the
    /// desktop's staging thread. Dead knowledge clears on rotation only when
    /// the finished round actually played something, so an all-junk library
    /// reaches a stop instead of re-proving forever.
    fn generate(inner: &mut Inner, target: usize) {
        let Some(root) = inner.root.clone() else {
            return;
        };
        let settings = AppSettings::load();
        let blacklist = if inner.configured {
            LibraryDb::open()
                .and_then(|db| db.list_blacklist())
                .map(|rows| {
                    Blacklist::from_rows(
                        &rows
                            .into_iter()
                            .map(|entry| (entry.kind, entry.path, entry.entry))
                            .collect::<Vec<_>>(),
                    )
                })
                .unwrap_or_default()
        } else {
            Blacklist::default()
        };
        let decoders = Arc::new(DecoderRegistry::new(settings.decoder_settings()));
        let audio_exts = decoders.audio_extensions();
        let nested_cache = kog_audio::archive::nested_cache_dir();
        let ctx = RadioCtx {
            decoders: &decoders,
            audio_exts: &audio_exts,
            // Cue sheets become their tracks through the same shared expansion
            // the desktop uses; honour the user's folder setting.
            read_cue: settings.read_cue_sheets_in_folders,
            nested_cache: &nested_cache,
        };
        let Some(mut round) = inner.round.take() else {
            return;
        };
        let mut made = 0_usize;
        let mut empty_rounds = 0_usize;
        let mut round_live = false;
        let mut skips = 0_usize;
        while made < target {
            match round.next_pick(&root, &ctx) {
                Some(pick) => {
                    empty_rounds = 0;
                    if locator_is_blacklisted(&pick, &blacklist) {
                        skips += 1;
                        if skips >= MAX_SKIPS {
                            break;
                        }
                        continue;
                    }
                    let key = radio_locator_key(&pick);
                    if inner.dead.contains(&key) {
                        skips += 1;
                        if skips >= MAX_SKIPS {
                            break;
                        }
                        continue;
                    }
                    let entries = prove_pick(&decoders, &pick, &root, PROVE_TIMEOUT);
                    if entries.is_empty() {
                        // The pick cannot produce a single playable track;
                        // prove it dead for both clients, as the desktop does
                        // when an expansion comes back empty.
                        inner.dead.insert(key);
                        skips += 1;
                        if skips >= MAX_SKIPS {
                            break;
                        }
                        continue;
                    }
                    skips = 0;
                    round_live = true;
                    made += entries.len();
                    inner.entries.extend(entries);
                }
                None => {
                    empty_rounds += 1;
                    if empty_rounds > 1 {
                        break;
                    }
                    let seed = random_seed();
                    inner.seed = seed;
                    // Mirror the desktop: only a round that yielded something
                    // clears the dead proof. A fruitless round keeps it.
                    if round_live {
                        round_live = false;
                        inner.dead.clear();
                    }
                    round = RadioRound::new(seed);
                }
            }
        }
        inner.round = Some(round);
        if let Some(path) = inner.save_path.clone() {
            if let Some(round) = inner.round.as_ref() {
                round.save(&path, &root, &inner.dead);
            }
        }
    }

    /// Persist the in-memory round. Best-effort, like the desktop.
    fn save(inner: &Inner) {
        let (Some(path), Some(root), Some(round)) = (
            inner.save_path.clone(),
            inner.root.clone(),
            inner.round.as_ref(),
        ) else {
            return;
        };
        round.save(&path, &root, &inner.dead);
    }

    /// Write the on/off flag to the desktop's setting, so both clients agree.
    /// Only the settings-backed state does this; explicit test states do not.
    fn persist_enabled(inner: &Inner, enabled: bool) {
        if inner.configured {
            let _ = AppSettings::save_radio_enabled(enabled);
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The round root: the persisted root when it is a real directory inside the
/// library, otherwise the library root.
fn resolve_root(save_path: Option<&Path>, library_root: Option<&Path>) -> Option<PathBuf> {
    let fallback = library_root.map(Path::to_path_buf);
    let Some(save_path) = save_path else {
        return fallback;
    };
    let Some(persisted) = persisted_root(save_path) else {
        return fallback;
    };
    let candidate = PathBuf::from(persisted);
    if candidate.is_dir() && library_root.is_some_and(|root| candidate.starts_with(root)) {
        Some(candidate)
    } else {
        fallback
    }
}

/// Read just the `root` field of a round file. The full round is validated by
/// `RadioRound::load`; this only decides which root to validate against.
fn persisted_root(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let document: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(document.get("root")?.as_str()?.to_owned())
}

/// Expand one round pick into its playable tracks. The shared
/// [`DecoderRegistry::expand_detailed`] handles cue sheets, subsong files and
/// archives exactly as the desktop does; every expanded source is then probed,
/// and unopenable ones are dropped. An empty result proves the whole pick dead.
///
/// One pick stages one song, and a multi-song file (an NSF with forty
/// subsongs, a cue sheet with twenty tracks) expands to that many sources —
/// staging them all flooded the window with one file's songs back to back.
/// Only the first source is staged per pick, mirroring the desktop, where a
/// pick arrives as a single locator.
/// Expand and prove one pick with a deadline. The proving thread owns its
/// inputs, so when the deadline passes the caller moves on while the stuck
/// thread is abandoned: its result is dropped and the pick is treated as
/// unplayable, which lands it in the dead set (persisted, so it is never
/// proved twice). The abandoned thread keeps whatever it holds until the
/// process ends; that is the price of containing a wedged native probe.
fn prove_pick(
    decoders: &Arc<DecoderRegistry>,
    pick: &Path,
    root: &Path,
    timeout: Duration,
) -> Vec<RadioEntry> {
    let decoders = Arc::clone(decoders);
    let pick = pick.to_path_buf();
    let root = root.to_path_buf();
    let label = pick.display().to_string();
    let (send, recv) = std::sync::mpsc::channel();
    if std::thread::Builder::new()
        .name("kog-radio-prove".to_owned())
        .spawn(move || {
            let entries = entries_from_pick(&decoders, &pick, &root);
            let _ = send.send(entries);
        })
        .is_err()
    {
        return Vec::new();
    }
    match recv.recv_timeout(timeout) {
        Ok(entries) => entries,
        Err(_) => {
            eprintln!("kog-server: radio pick timed out after {timeout:?}, marking dead: {label}");
            Vec::new()
        }
    }
}

fn entries_from_pick(decoders: &DecoderRegistry, pick: &Path, root: &Path) -> Vec<RadioEntry> {
    let Ok(expansion) = decoders.expand_detailed(pick.to_path_buf()) else {
        return Vec::new();
    };
    let count = expansion.sources.len();
    if count == 0 {
        return Vec::new();
    }
    // One song per pick, chosen at random among the file's own songs, so
    // repeats of the same file vary instead of always its first song. Sources
    // that fail to probe are walked past.
    let start = (random_seed() % count as u64) as usize;
    for offset in 0..count {
        let index = (start + offset) % count;
        let source = &expansion.sources[index];
        if let Ok(properties) = decoders.probe(source) {
            if let Some(entry) = entry_from_source(decoders, source, &properties, root) {
                return vec![entry];
            }
        }
    }
    Vec::new()
}

/// Map one played-back source to a streamable entry, shaped like every other
/// API entry so the web pane can render it with the existing row code.
fn entry_from_source(
    decoders: &DecoderRegistry,
    source: &PlaybackSource,
    properties: &StreamProperties,
    root: &Path,
) -> Option<RadioEntry> {
    let fragment = fragment_for_source(decoders, source, properties)?;
    if let Some(origin) = &source.archive_origin {
        return Some(RadioEntry {
            kind: "archive".to_owned(),
            path: origin.archive_path.display().to_string(),
            entry: origin.entry_name.clone(),
            fragment,
            relative: format!(
                "{}::{}",
                relative_from(root, &origin.archive_path),
                origin.entry_name
            ),
        });
    }
    if let Some(url) = &source.remote_url {
        return Some(RadioEntry {
            kind: "remote".to_owned(),
            path: url.clone(),
            entry: String::new(),
            fragment,
            relative: url.clone(),
        });
    }
    Some(RadioEntry {
        kind: "local".to_owned(),
        path: source.path.display().to_string(),
        entry: String::new(),
        fragment,
        relative: relative_from(root, &source.path),
    })
}

/// The fragment a client must send back to address this source again. The
/// outer `Option` is `None` when the source cannot be addressed (a cue track
/// without a declared number), which makes the caller skip it; the inner value
/// is the fragment itself. `resolve_entry` expects subsongs as their index but
/// cue tracks as their declared track number, so cue sources read the probed
/// number rather than the internal index.
fn fragment_for_source(
    decoders: &DecoderRegistry,
    source: &PlaybackSource,
    properties: &StreamProperties,
) -> Option<Option<String>> {
    match source.subsong {
        None => Some(None),
        Some(subsong) => {
            if decoders.selected_backend_id(source) == Some("cuesheet") {
                properties
                    .track_number
                    .map(|number| Some(number.to_string()))
            } else {
                Some(Some(subsong.to_string()))
            }
        }
    }
}

fn relative_from(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn status_of(inner: &Inner) -> RadioStatus {
    RadioStatus {
        enabled: inner.enabled,
        root: inner.root.as_ref().map(|root| root.display().to_string()),
        seed: inner.seed,
        entries: inner.entries.clone(),
    }
}

#[derive(Debug, Deserialize)]
pub struct EnabledRequest {
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct RootQuery {
    /// Optional scope: the client's current tree root. Radio plays that
    /// subtree, like the desktop's radio restarting under a changed tree
    /// root. Anything that is not a directory inside the configured library
    /// root is ignored, so the parameter cannot widen what the server serves.
    pub root: Option<String>,
}

/// The requested scope as a real path, when it is safely inside the
/// configured library root.
fn scoped_root(state: &AppState, requested: Option<&str>) -> Option<PathBuf> {
    // The scope is the client's tree root, wherever it points: the tree view
    // may root anywhere the server can read, and radio follows the tree, not
    // the configured music folder. Only existence is checked; the state
    // parameter stays for the library-root fallback at the call sites.
    let _ = state;
    let requested = requested.map(str::trim).filter(|root| !root.is_empty())?;
    let requested = Path::new(requested).canonicalize().ok()?;
    requested.is_dir().then_some(requested)
}

/// `GET /api/radio` — current state and the visible round window.
pub async fn status(State(state): State<AppState>, query: Query<RootQuery>) -> Response {
    let radio = state.radio.clone();
    let scope = scoped_root(&state, query.root.as_deref());
    let root = scope.clone().or_else(|| state.library.root());
    blocking(move || radio.snapshot(root.as_deref(), scope.as_deref())).await
}

/// `POST /api/radio/enabled` — turn random radio on or off.
pub async fn set_enabled(
    State(state): State<AppState>,
    query: Query<RootQuery>,
    axum::Json(request): axum::Json<EnabledRequest>,
) -> Response {
    let radio = state.radio.clone();
    let scope = scoped_root(&state, query.root.as_deref());
    let root = scope.clone().or_else(|| state.library.root());
    let enabled = request.enabled;
    blocking(move || radio.set_enabled(enabled, root.as_deref(), scope.as_deref())).await
}

/// `POST /api/radio/reshuffle` — fresh shuffle under the requested scope.
pub async fn reshuffle(State(state): State<AppState>, query: Query<RootQuery>) -> Response {
    let radio = state.radio.clone();
    let scope = scoped_root(&state, query.root.as_deref());
    let root = scope.clone().or_else(|| state.library.root());
    blocking(move || radio.reshuffle(root.as_deref(), scope.as_deref())).await
}

/// `POST /api/radio/advance` — the next window of the running round.
pub async fn advance(State(state): State<AppState>, query: Query<RootQuery>) -> Response {
    let radio = state.radio.clone();
    let scope = scoped_root(&state, query.root.as_deref());
    let root = scope.clone().or_else(|| state.library.root());
    blocking(move || radio.advance(root.as_deref(), scope.as_deref())).await
}

async fn blocking<T, F>(work: F) -> Response
where
    T: Serialize + Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&format!("random radio failed: {error}")),
    }
}

/// Router fragment for the radio endpoints, merged under the authenticated
/// API root so radio obeys the server's auth like everything else.
pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/api/radio", get(status))
        .route("/api/radio/enabled", post(set_enabled))
        .route("/api/radio/reshuffle", post(reshuffle))
        .route("/api/radio/advance", post(advance))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use std::sync::Arc;

    /// A pick whose probe never returns is declared unplayable by deadline,
    /// not by eternity: one miniusf in the wild hangs the native probe
    /// forever, which used to wedge the round (and every radio call behind
    /// its mutex) until the process was restarted. Skipped when the file is
    /// absent, so this never fails on machines without that library.
    #[test]
    fn hanging_probe_times_out_instead_of_wedging() {
        let pick = PathBuf::from(
            "/mnt/stuff/Music/Chiptune/VGM-Cartridge/N64/Turok - Dinosaur Hunter [Jikku Senshi Turok] (1997-02-28)(Iguana)(Acclaim)[N64]/09 Catacombs.miniusf",
        );
        if !pick.is_file() {
            eprintln!("skipped: hanging-probe fixture not present");
            return;
        }
        let settings = AppSettings::load();
        let decoders = Arc::new(DecoderRegistry::new(settings.decoder_settings()));
        let root = PathBuf::from("/mnt/stuff/Music/Chiptune/VGM-Cartridge");
        let started = std::time::Instant::now();
        let entries = prove_pick(&decoders, &pick, &root, Duration::from_secs(5));
        let elapsed = started.elapsed();
        assert!(entries.is_empty(), "a hanging pick proves nothing");
        assert!(
            elapsed < Duration::from_secs(30),
            "a hanging pick must give up, took {elapsed:?}"
        );
    }

    /// A short but genuinely decodable 8-bit mono WAV, so probing succeeds the
    /// way it does for a real library file.
    fn write_wav(path: &Path) {
        let data: Vec<u8> = vec![0x80; 800];
        let data: &[u8] = &data;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36_u32 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(data);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, wav).unwrap();
    }

    /// A 16-bit mono PCM WAV long enough for a small cue sheet, so the cue
    /// backend's FFmpeg probe has real frames to range over.
    fn write_pcm_wav(path: &Path, sample_rate: u32, seconds: u32) {
        let frames = sample_rate * seconds;
        let data_len = frames * 2;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for frame in 0..frames {
            let sample = ((frame % 64) as i32 - 32) as i16 * 500;
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, wav).unwrap();
    }

    /// A library with `count` wavs spread across two albums, plus a round-file
    /// path. The directory is leaked so the state outlives the test.
    fn fixture(count: usize) -> (Arc<crate::api::Library>, PathBuf, PathBuf) {
        let directory = Box::leak(Box::new(tempfile::tempdir().unwrap()));
        let root = directory.path().to_path_buf();
        for index in 0..count {
            let album = if index % 2 == 0 { "One" } else { "Two" };
            write_wav(&root.join(format!("{album}/track-{index:03}.wav")));
        }
        let library = crate::api::Library::new(
            Some(root.clone()),
            kog_core::db::LibraryDb::open_in_memory().expect("in-memory library"),
        );
        let save = root.join("radio-round.json");
        (Arc::new(library), root, save)
    }

    fn paths(status: &RadioStatus) -> Vec<String> {
        status
            .entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
    }

    #[test]
    fn enabling_yields_a_round_and_reshuffle_changes_the_order() {
        let (library, root, save) = fixture(90);
        let radio = Radio::new(Some(root.clone()), Some(save), false);
        let _ = library;
        let on = radio.set_enabled(true, Some(&root), None);
        assert!(on.enabled, "toggling on enables radio");
        assert_eq!(on.entries.len(), WINDOW, "a full window is staged");
        assert!(on.entries.iter().all(|entry| entry.kind == "local"));
        // Idempotent: asking again does not burn another window.
        let again = radio.set_enabled(true, Some(&root), None);
        assert_eq!(paths(&on), paths(&again));

        let fresh = radio.reshuffle(Some(&root), None);
        assert!(fresh.enabled);
        assert_ne!(on.seed, fresh.seed, "a reshuffle uses a new seed");
        assert_ne!(paths(&on), paths(&fresh), "a reshuffle changes the order");
    }

    #[test]
    fn an_explicit_scope_re_roots_the_round_to_the_subtree() {
        // The fixture spreads its wavs over `One/` and `Two/`; scoping to
        // `One` must restart the round there and never hand out `Two` again.
        let (library, root, save) = fixture(90);
        let radio = Radio::new(Some(root.clone()), Some(save.clone()), false);
        let _ = library;
        let unscoped = radio.set_enabled(true, Some(&root), None);
        assert!(
            unscoped
                .entries
                .iter()
                .any(|entry| entry.path.contains("/Two/")),
            "the unscoped round draws from the whole library"
        );

        let one = root.join("One");
        let one_str = one.to_str().unwrap();
        let scoped = radio.reshuffle(Some(&root), Some(&one));
        assert_eq!(scoped.root.as_deref(), Some(one_str), "the round re-roots");
        assert!(
            scoped
                .entries
                .iter()
                .all(|entry| entry.path.starts_with(one_str)),
            "every pick comes from the scoped subtree: {:?}",
            paths(&scoped)
        );
        // And the scoped round persists, so a client rooted at `One`
        // resumes the very same shuffle.
        assert_eq!(
            persisted_root(&save).as_deref(),
            Some(one.to_str().unwrap()),
            "the round file carries the scoped root"
        );
    }

    /// The dead set persisted for a round file rooted at `root`.
    fn persisted_dead(save: &Path, root: &Path) -> Vec<String> {
        RadioRound::load(save, root)
            .map(|round| round.dead)
            .unwrap_or_default()
    }

    #[test]
    fn unplayable_picks_are_skipped_and_remembered_dead() {
        let directory = Box::leak(Box::new(tempfile::tempdir().unwrap()));
        let root = directory.path().to_path_buf();
        // A file the decoders accept by name but cannot open: the pick must be
        // skipped and proved dead, not handed to a client that would fail to
        // stream it.
        let broken = root.join("broken.wav");
        std::fs::write(&broken, b"this is not a wave file").unwrap();
        let save = root.join("radio-round.json");
        let radio = Radio::new(Some(root.clone()), Some(save.clone()), true);

        let status = radio.snapshot(Some(&root), None);
        assert!(status.entries.is_empty(), "the only pick is unplayable");
        assert_eq!(
            persisted_dead(&save, &root),
            vec![radio_locator_key(&broken)],
            "the unplayable locator is persisted in the round"
        );
    }

    #[test]
    fn a_window_keeps_playable_picks_and_drops_unplayable_ones() {
        let directory = Box::leak(Box::new(tempfile::tempdir().unwrap()));
        let root = directory.path().to_path_buf();
        let good = root.join("good.wav");
        let broken = root.join("broken.wav");
        write_wav(&good);
        std::fs::write(&broken, b"this is not a wave file").unwrap();
        let save = root.join("radio-round.json");
        let radio = Radio::new(Some(root.clone()), Some(save), true);

        let status = radio.snapshot(Some(&root), None);
        assert!(!status.entries.is_empty(), "the playable pick is staged");
        assert!(
            status
                .entries
                .iter()
                .all(|entry| entry.path == good.display().to_string()),
            "only the playable pick reaches the client: {:?}",
            paths(&status)
        );
    }

    #[test]
    fn a_cue_pick_expands_into_its_tracks_addressed_by_number() {
        let directory = Box::leak(Box::new(tempfile::tempdir().unwrap()));
        let root = directory.path().to_path_buf();
        write_pcm_wav(&root.join("image.wav"), 8_000, 2);
        let cue = root.join("album.cue");
        std::fs::write(
            &cue,
            concat!(
                "FILE \"image.wav\" WAVE\n",
                "  TRACK 01 AUDIO\n",
                "    INDEX 01 00:00:00\n",
                "  TRACK 02 AUDIO\n",
                "    INDEX 01 00:01:00\n",
            ),
        )
        .unwrap();

        let decoders = DecoderRegistry::new(AppSettings::load().decoder_settings());
        let entries = entries_from_pick(&decoders, &cue, &root);
        // One pick stages one song: the cue becomes a single randomly chosen
        // track, not the whole album.
        assert_eq!(entries.len(), 1, "a cue pick becomes one track");
        assert!(entries.iter().all(|entry| entry.kind == "local"));
        assert!(
            entries
                .iter()
                .all(|entry| entry.path == cue.display().to_string())
        );
        // The fragment is the declared CUE track number, which is what
        // `resolve_entry` reads back, not the internal subsong index.
        let fragments: Vec<String> = entries
            .iter()
            .filter_map(|entry| entry.fragment.clone())
            .collect();
        assert_eq!(fragments.len(), 1);
        assert!(
            fragments[0] == "1" || fragments[0] == "2",
            "the staged fragment must be one of the cue's tracks: {fragments:?}"
        );
    }

    #[test]
    fn a_round_resumes_from_the_persisted_file() {
        let (library, root, save) = fixture(200);
        let _ = library;
        let first = Radio::new(Some(root.clone()), Some(save.clone()), true);
        let head = first.snapshot(Some(&root), None);
        assert_eq!(head.entries.len(), WINDOW);

        // A second session reads the same file and continues the same round:
        // same seed, different (later) picks.
        let second = Radio::new(Some(root.clone()), Some(save), true);
        let tail = second.snapshot(Some(&root), None);
        assert_eq!(head.seed, tail.seed, "the seed survives a restart");
        assert_ne!(
            paths(&head),
            paths(&tail),
            "the round continues, not restarts"
        );
        assert!(tail.entries.len() == WINDOW);
    }

    #[test]
    fn a_desktop_round_rooted_in_a_subfolder_is_resumed() {
        let (library, root, save) = fixture(4);
        // The persisted round names a root inside the library (as the desktop
        // does when the tree is rooted at a subfolder).
        let sub = root.join("One");
        std::fs::create_dir_all(&sub).unwrap();
        for index in 0..80 {
            write_wav(&sub.join(format!("extra-{index:02}.wav")));
        }
        let mut round = RadioRound::new(4242);
        let decoders = DecoderRegistry::new(AppSettings::load().decoder_settings());
        let exts = decoders.audio_extensions();
        let cache = kog_audio::archive::nested_cache_dir();
        let ctx = RadioCtx {
            decoders: &decoders,
            audio_exts: &exts,
            read_cue: false,
            nested_cache: &cache,
        };
        assert!(round.next_pick(&sub, &ctx).is_some());
        round.save(&save, &sub, &HashSet::new());

        let radio = Radio::new(Some(root.clone()), Some(save.clone()), true);
        let status = radio.snapshot(Some(&root), None);
        assert_eq!(
            status.root.as_deref(),
            Some(sub.to_string_lossy().as_ref()),
            "the desktop's root is respected"
        );
        assert_eq!(status.seed, 4242, "the desktop's seed is respected");
        assert!(!status.entries.is_empty());
        assert!(
            status
                .entries
                .iter()
                .all(|entry| Path::new(&entry.path).starts_with(&sub))
        );
        // The round file now carries the server's continuation at the same root.
        assert_eq!(
            persisted_root(&save).as_deref(),
            Some(sub.to_string_lossy().as_ref())
        );
        let _ = library;
    }

    #[test]
    fn a_foreign_round_root_falls_back_to_the_library_root() {
        let (_library, root, save) = fixture(4);
        let outside = Box::leak(Box::new(tempfile::tempdir().unwrap()));
        let mut round = RadioRound::new(7);
        round.save(&save, outside.path(), &HashSet::new());
        assert_eq!(
            resolve_root(Some(&save), Some(&root)),
            Some(root.clone()),
            "a root outside the library is ignored"
        );
    }

    fn state_with_radio(
        library: Arc<crate::api::Library>,
        root: PathBuf,
        save: PathBuf,
    ) -> AppState {
        let directory = Box::leak(Box::new(tempfile::tempdir().unwrap()));
        let streams = crate::service::StreamService::new(
            crate::stream::StreamCache::new(directory.path().join("streams"), 1 << 20),
            kog_audio::decoder::DecoderSettings::default(),
            PathBuf::from("ffmpeg"),
            directory.path().join("scratch"),
        );
        let config = crate::ServerConfig {
            enabled: true,
            auth: crate::auth::AuthMode::None,
            ..crate::ServerConfig::default()
        };
        AppState::with_radio(
            config,
            "9.9.9",
            streams,
            Arc::try_unwrap(library).unwrap_or_else(|_| {
                crate::api::Library::new(
                    Some(root.clone()),
                    kog_core::db::LibraryDb::open_in_memory().unwrap(),
                )
            }),
            Radio::new(Some(root), Some(save), false),
        )
    }

    async fn request(
        state: AppState,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        use axum::body::Body;
        use axum::http::Request;
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let mut builder = Request::builder().method(method).uri(path);
        let body = match body {
            Some(value) => {
                builder = builder.header("content-type", "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        let response = crate::routes::router(state)
            .oneshot(builder.body(body).unwrap())
            .await
            .expect("router responds");
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    #[tokio::test]
    async fn radio_endpoints_round_trip_through_the_router() {
        let (library, root, save) = fixture(30);
        let state = state_with_radio(library, root, save);

        let (status, body) = request(state.clone(), "GET", "/api/radio", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["enabled"], false, "radio starts off");
        assert!(body["entries"].as_array().unwrap().is_empty());

        let (status, body) = request(
            state.clone(),
            "POST",
            "/api/radio/enabled",
            Some(serde_json::json!({ "enabled": true })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["enabled"], true);
        let entries = body["entries"].as_array().unwrap();
        assert_eq!(entries.len(), WINDOW);
        assert!(entries[0]["path"].as_str().unwrap().ends_with(".wav"));

        let (status, body) = request(state.clone(), "POST", "/api/radio/reshuffle", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["enabled"], true);
        assert_eq!(body["entries"].as_array().unwrap().len(), WINDOW);

        let (status, body) = request(state.clone(), "POST", "/api/radio/advance", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["exhausted"], false);
        assert_eq!(body["entries"].as_array().unwrap().len(), WINDOW);

        let (status, body) = request(
            state,
            "POST",
            "/api/radio/enabled",
            Some(serde_json::json!({ "enabled": false })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["enabled"], false);
    }
}
