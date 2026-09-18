//! Random Radio hierarchical descent: exhaustive shuffled round-robin picks
//! with (seed, cursor) state per visited folder.
//!
//! Each pick descends from the music root. Every folder shuffles its turns
//! (its directly-held files as one group, plus one turn per child folder)
//! with a seed derived from the round seed and the folder itself, serves the
//! next turn in that order, and descends into child folders the same way.
//! Only cursor positions persist per folder; the shuffled orders are
//! recomputed functionally, so memory stays flat no matter how large the
//! library is. Exhausted folders drop out of rotation. When the root is
//! exhausted every song has played exactly once, and a new round starts with
//! a fresh seed, making every round a unique full-coverage shuffle.
//!
//! Small folders would otherwise vanish for the rest of a long round once
//! played out, so spent folders replay after every pick they served has aged
//! past FOLDER_REPLAY_SPACING picks: present all round, never loopy (the
//! spacing is weeks of listening). Folders that never yielded anything
//! rest forever. Rounds shorter than the spacing rotate before any replay,
//! so small libraries behave as pure exhaustive rounds.
//!
//! Rounds persist across sessions (seed, cursors, replay ages, dead keys),
//! so restarts and re-toggles continue the descent instead of replaying
//! openers. The explicit reshuffle action starts a fresh shuffle but keeps
//! unplayability knowledge; rotation clears it so fixed files re-prove
//! themselves.
//!
//! Archives descend exactly like folders, nested archives included (up to the
//! shared 4-level safety cap). Unlistable archives contribute nothing rather
//! than burning picks on doomed expansions.
//!
//! Library mutations mid-round are tolerated, not transactional: orders are
//! recomputed per visit, so added files may wait for the next round and a
//! cursor can step past a deleted file. The next round always sees a clean
//! tree.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::decoder::{DecoderRegistry, DecoderSettings};

fn hash_with_seed(seed: u64, bytes: &[u8]) -> u64 {
    let mut salted = seed.to_le_bytes().to_vec();
    salted.extend_from_slice(bytes);
    crate::cover_art::fnv1a64(&salted)
}

pub fn random_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|age| age.as_secs() ^ u64::from(age.subsec_nanos()))
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
}

/// Salt distinguishing file order from turn order inside one folder.
const FILE_ORDER_SALT: u64 = 0x9E37_79B9_7F4A_7C15;

/// Folders that exhaust replay their songs once every pick here has aged
/// past this many picks. Small genres stay present all round instead of
/// vanishing after the first cycles, while no song ever feels loopy: 500
/// picks is weeks of listening. Rounds shorter than this rotate first, so
/// small libraries are unaffected. Tune here.
const FOLDER_REPLAY_SPACING: u64 = 500;

/// User-blacklisted songs and folders, shared live with the UI so menu
/// changes apply without restarting staging. Songs match pick locator
/// keys exactly; folders match by path prefix (archive members also
/// match on their outer file's folders).
#[derive(Clone, Debug, Default)]
pub struct Blacklist {
    pub songs: HashSet<String>,
    pub folders: Vec<PathBuf>,
}

impl Blacklist {
    /// Build from `(kind, path, entry)` rows. Paths resolve best-effort
    /// so tree, pane, and descent forms agree even across symlinks;
    /// missing files keep their raw strings.
    pub fn from_rows(rows: &[(String, String, String)]) -> Self {
        let mut songs = HashSet::new();
        let mut folders = Vec::new();
        for (kind, path, entry) in rows {
            if kind == "folder" {
                folders.push(resolve_path(path));
            } else if entry.is_empty() {
                songs.insert(resolve_path(path).display().to_string());
            } else {
                // No spaces: mirrors radio_locator_key exactly.
                songs.insert(format!("{}::{}", resolve_path(path).display(), entry));
            }
        }
        Self { songs, folders }
    }
}

fn resolve_path(path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    crate::track::canonical_path(&candidate).unwrap_or(candidate)
}

pub fn locator_is_blacklisted(locator: &Path, blacklist: &Blacklist) -> bool {
    if blacklist.songs.is_empty() && blacklist.folders.is_empty() {
        return false;
    }
    // Resolve the same way as insertion so symlinked trees match either
    // form; archive members match on their canonical outer + member key
    // as well as on outer-folder prefixes.
    let resolved = resolve_path(&locator.to_string_lossy());
    if let Ok(Some(location)) = crate::archive::tree_location(&resolved) {
        let outer = resolve_path(&location.archive.to_string_lossy());
        if blacklist
            .songs
            .contains(&format!("{}::{}", outer.display(), location.entry))
        {
            return true;
        }
        if blacklist
            .folders
            .iter()
            .any(|folder| outer.starts_with(folder))
        {
            return true;
        }
    } else if blacklist
        .songs
        .contains(&resolved.display().to_string())
    {
        return true;
    }
    blacklist
        .folders
        .iter()
        .any(|folder| resolved.starts_with(folder))
}

/// Picks produced by the staging thread: locators in round order, Empty
/// when the library yielded nothing across two consecutive rounds, Barren
/// when everything reachable already failed this session.
#[derive(Debug)]
pub enum StagingResponse {
    Pick(PathBuf),
    Empty,
    Barren,
}

/// Staging thread body: own the round, its decoder set, and its cursors;
/// descend picks and post them until cancelled or the receiver is gone. The
/// bounded channel paces production: a full channel blocks the thread, so
/// nothing is ever staged faster than the UI consumes. Round exhaustion
/// rotates to a fresh seed silently; a second consecutive exhaustion ends
/// the thread with Empty. Locators the UI already proved dead are skipped
/// without expansion (their turns stay consumed, so orders never shift).
pub fn run_staging(
    root: PathBuf,
    settings: DecoderSettings,
    read_cue: bool,
    nested_cache: PathBuf,
    initial: RoundInitial,
    save_path: Option<PathBuf>,
    dead: std::sync::Arc<std::sync::Mutex<HashSet<String>>>,
    blacklist: std::sync::Arc<std::sync::Mutex<Blacklist>>,
    picks: std::sync::mpsc::SyncSender<StagingResponse>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    // A library of pure junk would spin rounds of skips forever: cap
    // consecutive skips, then report Barren and park.
    const MAX_SKIPS: usize = 16384;
    let decoders = DecoderRegistry::new(settings);
    let exts = decoders.audio_extensions();
    let ctx = RadioCtx {
        decoders: &decoders,
        audio_exts: &exts,
        read_cue,
        nested_cache: &nested_cache,
    };
    for key in &initial.dead {
        dead.lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(key.clone());
    }
    let save = |round: &RadioRound| {
        let Some(path) = save_path.as_deref() else {
            return;
        };
        let snapshot = dead
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        round.save(path, &root, &snapshot);
    };
    let mut round = RadioRound::restore(initial);
    // Consecutive rounds with zero yields of any kind end the thread; a
    // round that yields (live or skipped) proves content and resets both.
    let mut empty_rounds = 0_usize;
    let mut round_yielded = false;
    let mut round_live = false;
    let mut skips = 0_usize;
    let mut since_save = 0_usize;
    // Every exit path below saves first (save() is best-effort), except the
    // receiver-gone path where nobody will ever read the file... it still
    // saves: teardown drops the receiver, and the next toggle loads.
    macro_rules! exit {
        () => {{
            save(&round);
            return;
        }};
    }
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            exit!();
        }
        match round.next_pick(&root, &ctx) {
            Some(locator) => {
                round_yielded = true;
                let known_dead = dead
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .contains(&radio_locator_key(&locator));
                let blacklisted = locator_is_blacklisted(
                    &locator,
                    &blacklist
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()),
                );
                if known_dead || blacklisted {
                    skips += 1;
                    if skips >= MAX_SKIPS {
                        let _ = picks.send(StagingResponse::Barren);
                        exit!();
                    }
                    continue;
                }
                skips = 0;
                round_live = true;
                if picks.send(StagingResponse::Pick(locator)).is_err() {
                    exit!();
                }
                since_save += 1;
                if since_save >= SAVE_EVERY_PICKS {
                    since_save = 0;
                    save(&round);
                }
            }
            None => {
                if round_yielded {
                    empty_rounds = 0;
                } else {
                    empty_rounds += 1;
                }
                if empty_rounds > 1 {
                    let _ = picks.send(StagingResponse::Empty);
                    exit!();
                }
                // Fresh round, fresh shuffle. Dead knowledge clears only
                // when the old round actually played something: a fruitless
                // round keeps its proof, so all-junk libraries still reach
                // Barren instead of re-proving forever. Saved at once so a
                // crash cannot resurrect stale dead keys.
                round = RadioRound::new(random_seed());
                round_yielded = false;
                if round_live {
                    round_live = false;
                    dead.lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .clear();
                }
                save(&round);
            }
        }
    }
}
/// Per-folder radio state: positions in the functionally-shuffled orders.
/// That is the only thing a round remembers; orders are recomputed.
#[derive(Clone, Copy, Default)]
pub struct RadioCursor {
    entry: usize,
    file: usize,
    files_done: bool,
    done: bool,
    /// Pick counter at this folder's last yield, if it ever yielded. Drives
    /// replay spacing; folders that never yielded rest forever.
    last_yield: Option<u64>,
}

impl RadioCursor {
    fn flags(self) -> u64 {
        (u64::from(self.files_done)) | (u64::from(self.done) << 1)
    }

    fn from_parts(entry: u64, file: u64, flags: u64, last_yield: Option<u64>) -> Option<Self> {
        let entry = usize::try_from(entry).ok()?;
        let file = usize::try_from(file).ok()?;
        if flags > 3 {
            return None;
        }
        Some(Self {
            entry,
            file,
            files_done: flags & 1 != 0,
            done: flags & 2 != 0,
            last_yield,
        })
    }
}

/// Starting state for a round: fresh (new seed, empty cursors) or restored
/// from disk (same seed and positions, so picks continue where the last
/// session left off instead of replaying its openers).
pub struct RoundInitial {
    pub seed: u64,
    pub counter: u64,
    pub cursors: HashMap<String, RadioCursor>,
    pub dead: Vec<String>,
}

impl RoundInitial {
    pub fn fresh(seed: u64) -> Self {
        Self {
            seed,
            counter: 0,
            cursors: HashMap::new(),
            dead: Vec::new(),
        }
    }
}

/// One round of exhaustive picks: a seed, a pick counter, and a cursor per
/// visited folder. The counter stamps every yield and ages replay spacing.
pub struct RadioRound {
    seed: u64,
    counter: u64,
    cursors: HashMap<String, RadioCursor>,
}

/// Round persistence format version. Bump when the layout changes; older
/// files fail validation and start fresh rounds instead of misreading.
const ROUND_FILE_VERSION: u64 = 1;
/// Cap on persisted dead keys: bounds the file while staying far above any
/// realistic junk count. Truncated entries simply re-prove themselves.
const MAX_PERSISTED_DEAD: usize = 5000;
/// Save cadence in staged picks: bounds loss on quit/crash to a few songs.
const SAVE_EVERY_PICKS: usize = 25;

/// Everything descent needs that is not the round itself. Built once per
/// refill so member filtering does not rebuild the extension set per folder.
pub struct RadioCtx<'a> {
    pub decoders: &'a DecoderRegistry,
    pub audio_exts: &'a HashSet<String>,
    pub read_cue: bool,
    pub nested_cache: &'a Path,
}

impl RadioRound {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            counter: 0,
            cursors: HashMap::new(),
        }
    }

    pub fn restore(initial: RoundInitial) -> Self {
        Self {
            seed: initial.seed,
            counter: initial.counter,
            cursors: initial.cursors,
        }
    }

    /// Persist this round: seed, pick counter, cursor positions with replay
    /// ages, and dead keys as JSON. Best-effort by design; callers ignore
    /// failures and keep playing memory-only. Atomic write (temp + rename)
    /// so a crash never leaves a half-written round behind.
    pub fn save(&self, path: &Path, root: &Path, dead: &HashSet<String>) {
        let mut cursors = serde_json::Map::with_capacity(self.cursors.len());
        for (node, cursor) in &self.cursors {
            cursors.insert(
                node.clone(),
                serde_json::Value::Array(vec![
                    serde_json::Value::from(cursor.entry as u64),
                    serde_json::Value::from(cursor.file as u64),
                    serde_json::Value::from(cursor.flags()),
                    match cursor.last_yield {
                        Some(last) => serde_json::Value::from(last),
                        None => serde_json::Value::Null,
                    },
                ]),
            );
        }
        let mut dead_keys: Vec<&String> = dead.iter().collect();
        dead_keys.sort();
        dead_keys.truncate(MAX_PERSISTED_DEAD);
        let document = serde_json::Value::Object(serde_json::Map::from_iter([
            ("v".to_owned(), serde_json::Value::from(ROUND_FILE_VERSION)),
            (
                "root".to_owned(),
                serde_json::Value::from(root.display().to_string()),
            ),
            ("seed".to_owned(), serde_json::Value::from(self.seed)),
            ("counter".to_owned(), serde_json::Value::from(self.counter)),
            ("cursors".to_owned(), serde_json::Value::Object(cursors)),
            (
                "dead".to_owned(),
                serde_json::Value::Array(
                    dead_keys
                        .into_iter()
                        .map(|key| serde_json::Value::from(key.clone()))
                        .collect(),
                ),
            ),
        ]));
        let text = match serde_json::to_string(&document) {
            Ok(text) => text,
            Err(_) => return,
        };
        let Some(parent) = path.parent() else {
            return;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let partial = path.with_extension("part");
        if std::fs::write(&partial, text).is_err() {
            return;
        }
        let _ = std::fs::rename(&partial, path);
    }

    /// Load a persisted round for `root`: validated version, matching root,
    /// and well-formed cursors, or None (fresh round) on anything else.
    pub fn load(path: &Path, root: &Path) -> Option<RoundInitial> {
        let text = std::fs::read_to_string(path).ok()?;
        let document: serde_json::Value = serde_json::from_str(&text).ok()?;
        let object = document.as_object()?;
        if object.get("v")?.as_u64()? != ROUND_FILE_VERSION {
            return None;
        }
        if object.get("root")?.as_str()? != root.display().to_string() {
            return None;
        }
        let seed = object.get("seed")?.as_u64()?;
        let counter = object.get("counter")?.as_u64()?;
        let mut cursors = HashMap::new();
        for (node, packed) in object.get("cursors")?.as_object()? {
            let packed = packed.as_array()?;
            if packed.len() != 4 {
                return None;
            }
            let last_yield = match &packed[3] {
                serde_json::Value::Null => None,
                value => Some(value.as_u64()?),
            };
            let cursor = RadioCursor::from_parts(
                packed[0].as_u64()?,
                packed[1].as_u64()?,
                packed[2].as_u64()?,
                last_yield,
            )?;
            if node.is_empty() {
                return None;
            }
            cursors.insert(node.clone(), cursor);
        }
        let mut dead = Vec::new();
        if let Some(entries) = object.get("dead").and_then(|value| value.as_array()) {
            for entry in entries {
                dead.push(entry.as_str()?.to_owned());
            }
        }
        Some(RoundInitial {
            seed,
            counter,
            cursors,
            dead,
        })
    }

    /// Next pick from the music root: descend level by level, or None when
    /// every song has played once and the round is exhausted. Successful
    /// picks advance the pick counter that ages replay spacing.
    pub fn next_pick(&mut self, root: &Path, ctx: &RadioCtx) -> Option<PathBuf> {
        let pick = self.descend(&fs_node(root), ctx);
        if pick.is_some() {
            self.counter += 1;
        }
        pick
    }

    fn node_seed(&self, node: &str) -> u64 {
        hash_with_seed(self.seed, node.as_bytes())
    }

    /// Serve one pick from `node`: cycle its turns in shuffled order,
    /// skipping turns that already exhausted themselves this round. Spent
    /// folders rest until every pick they served has aged past the replay
    /// spacing, then start a fresh sub-cycle — except folders that never
    /// yielded (all dead or always empty), which rest forever.
    fn descend(&mut self, node: &str, ctx: &RadioCtx) -> Option<PathBuf> {
        // Spent folders stay spent, except ones whose last yield aged past
        // the replay spacing: those clear into a fresh sub-cycle below.
        // Folders that never yielded (all dead or always empty) never qualify.
        let reenter = match self.cursors.get(node) {
            None => false,
            Some(cursor) if !cursor.done => false,
            Some(cursor) => cursor
                .last_yield
                .is_some_and(|last| self.counter.saturating_sub(last) > FOLDER_REPLAY_SPACING),
        };
        if reenter {
            let cursor = self.cursors.get_mut(node).expect("cursor visited");
            cursor.done = false;
            cursor.files_done = false;
            cursor.entry = 0;
            cursor.file = 0;
            cursor.last_yield = None;
        } else if self.cursors.get(node).is_some_and(|cursor| cursor.done) {
            return None;
        }
        let seed = self.node_seed(node);
        let mut listing = list_radio_node(node, ctx);
        listing.files.sort_by(|left, right| {
            hash_with_seed(seed ^ FILE_ORDER_SALT, left.0.as_bytes())
                .cmp(&hash_with_seed(seed ^ FILE_ORDER_SALT, right.0.as_bytes()))
                .then_with(|| left.1.cmp(&right.1))
        });
        // Turn 0 is the files group; turns 1.. are child folders.
        let mut order: Vec<usize> = Vec::with_capacity(listing.dirs.len() + 1);
        if !listing.files.is_empty() {
            order.push(0);
        }
        order.extend(1..=listing.dirs.len());
        order.sort_by(|&left, &right| {
            let name = |turn: usize| {
                if turn == 0 {
                    ""
                } else {
                    listing.dirs[turn - 1].0.as_str()
                }
            };
            hash_with_seed(seed, name(left).as_bytes())
                .cmp(&hash_with_seed(seed, name(right).as_bytes()))
                .then_with(|| left.cmp(&right))
        });
        if order.is_empty() {
            self.cursors.entry(node.to_owned()).or_default().done = true;
            return None;
        }
        for _ in 0..order.len() {
            let turn = {
                let cursor = self.cursors.entry(node.to_owned()).or_default();
                let turn = order[cursor.entry % order.len()];
                cursor.entry += 1;
                turn
            };
            if turn == 0 {
                let cursor = self.cursors.get_mut(node).expect("cursor inserted");
                if cursor.files_done {
                    continue;
                }
                if cursor.file < listing.files.len() {
                    let locator = listing.files[cursor.file].1.clone();
                    cursor.file += 1;
                    if cursor.file >= listing.files.len() {
                        cursor.files_done = true;
                    }
                    cursor.last_yield = Some(self.counter);
                    return Some(locator);
                }
                cursor.files_done = true;
            } else {
                // Done children are visited, not skipped: only the child
                // itself can decide it rested long enough to replay.
                let child = listing.dirs[turn - 1].1.clone();
                if let Some(locator) = self.descend(&child, ctx) {
                    self.cursors
                        .get_mut(node)
                        .expect("cursor visited")
                        .last_yield = Some(self.counter);
                    return Some(locator);
                }
            }
        }
        self.cursors.get_mut(node).expect("cursor inserted").done = true;
        None
    }
}

/// Node addresses. Filesystem folders address by absolute path; archive
/// interiors by outer archive, nesting depth, and within-archive prefix (""
/// for the archive root). `\0` separates fields; names cannot contain it.
fn fs_node(path: &Path) -> String {
    format!("fs\x00{}", path.display())
}

fn archive_node(outer: &Path, depth: u32, prefix: &str) -> String {
    format!("ar\x00{}\x00{depth}\x00{prefix}", outer.display())
}

/// One folder's children: playable files with locators, child folders with
/// node addresses. Both carry their sort name for the seeded shuffle.
#[derive(Default)]
struct RadioListing {
    files: Vec<(String, PathBuf)>,
    dirs: Vec<(String, String)>,
}

/// Whether a filesystem file is eligible for radio rotation: a real audio
/// candidate the decoders accept. Playlists never qualify (one pick could
/// enqueue hundreds of tracks); cue sheets follow the folder setting.
fn is_radio_file(path: &Path, read_cue: bool, decoders: &DecoderRegistry) -> bool {
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
    {
        return false;
    }
    if !is_random_candidate(path, read_cue) {
        return false;
    }
    decoders.accepts_path(path)
}

/// Whether an archive member name is eligible: same candidate rules, with
/// the playable-extension set standing in for the decoder backends (which
/// only inspect extensions, except rare magic-checked ones expansion itself
/// will still refuse).
fn is_radio_member(name: &str, read_cue: bool, audio_exts: &HashSet<String>) -> bool {
    if !is_random_candidate(Path::new(name), read_cue) {
        return false;
    }
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            audio_exts.contains(&extension.to_ascii_lowercase())
                || crate::archive::is_path(Path::new(name))
        })
}

/// Stable identity for playlist deduplication: plain paths as-is, archive
/// tracks as `outer :: member`. Mirrors radio_locator_key so both sides of
/// the comparison speak the same language (lexically, like playlist purge).
/// Lives here (not the controller) so the staging thread and the UI share
/// one scheme for the dead set.
pub fn radio_track_key(track: &crate::track::Track) -> String {
    if let Some(origin) = &track.source.archive_origin {
        return format!("{}::{}", origin.archive_path.display(), origin.entry_name);
    }
    if let Some(url) = &track.source.remote_url {
        return url.clone();
    }
    track.source.path.display().to_string()
}

/// Stable identity for a catalog locator: plain paths as-is, archive member
/// URLs parsed back to `outer :: member`.
pub fn radio_locator_key(path: &Path) -> String {
    if let Ok(Some(location)) = crate::archive::tree_location(path) {
        return format!("{}::{}", location.archive.display(), location.entry);
    }
    path.display().to_string()
}

/// Shuffled play order for one multi-song file: uniform multi-song
/// expansions (every track carries a distinct subsong index, as NSF and
/// friends produce) come back in hash order, so radio stages a single
/// shuffled pick now and defers the rest for spaced staging. Anything else
/// (single tracks, missing or duplicated indices) returns None and stages
/// whole, exactly as before.
pub fn shuffle_subsong_order(file_key: &str, subsongs: &[u32]) -> Option<Vec<u32>> {
    if subsongs.len() < 2 {
        return None;
    }
    let mut unique = subsongs.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.len() != subsongs.len() {
        return None;
    }
    unique.sort_by(|left, right| {
        let mut left_key = file_key.as_bytes().to_vec();
        left_key.extend_from_slice(&left.to_le_bytes());
        let mut right_key = file_key.as_bytes().to_vec();
        right_key.extend_from_slice(&right.to_le_bytes());
        hash_with_seed(FILE_ORDER_SALT, &left_key)
            .cmp(&hash_with_seed(FILE_ORDER_SALT, &right_key))
            .then_with(|| left.cmp(right))
    });
    Some(unique)
}

/// Whether a discovered file is eligible for random queueing: regular
/// audio, cue sheets (when enabled), and archives, which expand through the
/// normal add path. Playlists are excluded because one pick could enqueue
/// hundreds of tracks.
fn is_random_candidate(path: &Path, read_cue_sheets: bool) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "m3u" | "m3u8" | "pls") {
        return false;
    }
    if extension == "cue" {
        return read_cue_sheets;
    }
    true
}

fn list_radio_node(node: &str, ctx: &RadioCtx) -> RadioListing {
    let mut listing = RadioListing::default();
    let Some((tag, rest)) = node.split_once('\0') else {
        return listing;
    };
    if tag == "fs" {
        list_fs_dir(Path::new(rest), ctx, &mut listing);
    } else if tag == "ar" {
        let mut parts = rest.splitn(3, '\0');
        let (Some(outer), Some(depth), Some(prefix)) = (parts.next(), parts.next(), parts.next())
        else {
            return listing;
        };
        let Ok(depth) = depth.parse::<u32>() else {
            return listing;
        };
        list_archive_interior(Path::new(outer), depth, prefix, ctx, &mut listing);
    }
    listing
}

fn list_fs_dir(dir: &Path, ctx: &RadioCtx, listing: &mut RadioListing) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(kind) = std::fs::symlink_metadata(&path).map(|metadata| metadata.file_type()) else {
            continue;
        };
        if kind.is_symlink() || (!kind.is_file() && !kind.is_dir()) {
            continue;
        }
        if kind.is_dir() {
            listing.dirs.push((name, fs_node(&path)));
            continue;
        }
        if kog_core::media_path::is_metadata(&path) {
            continue;
        }
        if crate::archive::is_path(&path) {
            listing.dirs.push((name, archive_node(&path, 1, "")));
            continue;
        }
        if !is_radio_file(&path, ctx.read_cue, ctx.decoders) {
            continue;
        }
        listing.files.push((name, path));
    }
}

/// One archive-interior folder: resolve its container (materializing nested
/// archives through the stable cache), scope the member list to `prefix`,
/// and classify immediate children. Explicit `name/` directory entries win
/// over same-named archives, mirroring extraction; anything deeper without
/// an explicit entry descends as a folder, exactly like expansion resolves
/// it.
fn list_archive_interior(
    outer: &Path,
    depth: u32,
    prefix: &str,
    ctx: &RadioCtx,
    listing: &mut RadioListing,
) {
    // Resolve to the container holding this folder: the outer archive for
    // the root, a cached materialization deeper down. Members list relative
    // to that container, so the unconsumed leaf remainder ("" at a container
    // root) is the only scope left to apply.
    let resolved = match crate::archive::resolve_archive_chain(outer, prefix, ctx.nested_cache) {
        Ok(resolved) => resolved,
        Err(_) => return,
    };
    let members = match crate::archive::list_archive_names(&resolved.container) {
        Ok(members) => members,
        Err(_) => return,
    };
    let leaf = resolved.leaf.trim_end_matches('/');
    struct Child {
        file: bool,
        explicit_dir: bool,
        deeper: bool,
    }
    let mut children: HashMap<String, Child> = HashMap::new();
    for member in &members {
        let normalized = member.replace('\\', "/");
        let relative = if leaf.is_empty() {
            normalized.as_str()
        } else {
            match normalized.strip_prefix(leaf) {
                Some(rest) if rest.starts_with('/') => &rest[1..],
                _ => continue,
            }
        };
        let trimmed = relative.trim_end_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        let (child, rest) = match trimmed.split_once('/') {
            Some((child, _)) => (child, true),
            None => (trimmed, false),
        };
        let entry = children.entry(child.to_owned()).or_insert_with(|| Child {
            file: false,
            explicit_dir: false,
            deeper: false,
        });
        if rest {
            entry.deeper = true;
        } else if member.ends_with('/') {
            entry.explicit_dir = true;
        } else {
            entry.file = true;
        }
    }
    for (child, info) in &children {
        let full = if prefix.is_empty() {
            (*child).clone()
        } else {
            format!("{prefix}/{child}")
        };
        if child.starts_with('.') || kog_core::media_path::is_metadata(Path::new(&full)) {
            continue;
        }
        if info.explicit_dir {
            listing
                .dirs
                .push(((*child).clone(), archive_node(outer, depth, &full)));
        } else if info.file && crate::archive::is_path(Path::new(child)) {
            if depth + 1 <= crate::archive::MAX_NESTED_DEPTH as u32 {
                listing
                    .dirs
                    .push(((*child).clone(), archive_node(outer, depth + 1, &full)));
            }
        } else if info.deeper {
            listing
                .dirs
                .push(((*child).clone(), archive_node(outer, depth, &full)));
        } else if info.file {
            if !is_radio_member(&full, ctx.read_cue, ctx.audio_exts) {
                continue;
            }
            listing.files.push((
                (*child).clone(),
                crate::archive::member_url(outer, &full, false),
            ));
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
mod tests {
    use super::*;

    fn ctx<'a>(
        decoders: &'a DecoderRegistry,
        exts: &'a HashSet<String>,
        cache: &'a Path,
    ) -> RadioCtx<'a> {
        RadioCtx {
            decoders,
            audio_exts: exts,
            read_cue: false,
            nested_cache: cache,
        }
    }

    /// Extension universe the expansion path accepts, minus playlists
    /// (which radio excludes by policy, not capability).
    fn test_exts() -> HashSet<String> {
        ["flac", "mp3", "wav", "mid", "midi"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn drain(round: &mut RadioRound, root: &Path, ctx: &RadioCtx) -> Vec<PathBuf> {
        let mut picks = Vec::new();
        while picks.len() <= 4096 {
            match round.next_pick(root, ctx) {
                Some(pick) => picks.push(pick),
                None => break,
            }
        }
        picks
    }
    #[test]
    fn round_save_load_roundtrip_restores_positions() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        for name in ["a", "b"] {
            std::fs::create_dir(root.join(name)).unwrap();
            for index in 0..3 {
                std::fs::write(root.join(format!("{name}/track-{index}.flac")), []).unwrap();
            }
        }
        let decoders = DecoderRegistry::default();
        let exts = test_exts();
        let cache = root.join("cache");
        let context = ctx(&decoders, &exts, &cache);
        // Drain part of a round, persist, restore into a fresh round, and
        // prove the combined sequence equals one uninterrupted round.
        let mut first = RadioRound::new(11);
        let mut head = Vec::new();
        for _ in 0..4 {
            head.push(first.next_pick(root, &context).expect("pick"));
        }
        let path = root.join("round.json");
        let mut dead = HashSet::new();
        dead.insert("stale-key".to_owned());
        first.save(&path, root, &dead);
        let loaded = RadioRound::load(&path, root).expect("round loads");
        assert_eq!(loaded.dead, vec!["stale-key".to_owned()]);
        assert_eq!(loaded.counter, 4, "pick counter persists");
        assert!(
            loaded
                .cursors
                .values()
                .any(|cursor| cursor.last_yield.is_some()),
            "replay ages persist"
        );
        let mut second = RadioRound::restore(loaded);
        let mut tail = Vec::new();
        while let Some(pick) = second.next_pick(root, &context) {
            tail.push(pick);
            if tail.len() > 64 {
                panic!("round never exhausted");
            }
        }
        let mut reference = RadioRound::new(11);
        let mut full = Vec::new();
        while let Some(pick) = reference.next_pick(root, &context) {
            full.push(pick);
        }
        head.extend(tail);
        assert_eq!(head, full, "restored round continues exactly");
    }

    #[test]
    fn round_load_rejects_garbage_and_foreign_roots() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        let path = root.join("round.json");
        std::fs::write(&path, b"not json").unwrap();
        assert!(RadioRound::load(&path, root).is_none());
        std::fs::write(&path, br#"{"v":999,"root":"/x","seed":1,"cursors":{}}"#).unwrap();
        assert!(RadioRound::load(&path, root).is_none());
        std::fs::write(
            &path,
            format!(r#"{{"v":1,"root":"/elsewhere","seed":1,"cursors":{{}}}}"#).as_bytes(),
        )
        .unwrap();
        assert!(RadioRound::load(&path, root).is_none());
        // Malformed cursor entries fail closed too.
        std::fs::write(
            &path,
            format!(
                r#"{{"v":1,"root":"{}","seed":1,"cursors":{{"n":[1]}}}}"#,
                root.display()
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(RadioRound::load(&path, root).is_none());
    }

    #[test]
    fn radio_keys_agree_between_tracks_and_locators() {
        use crate::decoder::PlaybackSource;
        use crate::track::Track;
        let file = PathBuf::from("/music/song.flac");
        let file_track = Track {
            source: PlaybackSource::from_path(file.clone()),
            ..Track::default()
        };
        assert_eq!(radio_track_key(&file_track), radio_locator_key(&file));
        let mut archived = Track {
            source: PlaybackSource::from_path(PathBuf::from("/music/pack.zip/Disc/a.wav")),
            ..Track::default()
        };
        archived
            .source
            .set_archive_origin(PathBuf::from("/music/pack.zip"), "Disc/a.wav".to_owned());
        let locator = crate::archive::member_url(Path::new("/music/pack.zip"), "Disc/a.wav", false);
        assert_eq!(radio_track_key(&archived), radio_locator_key(&locator));
        let remote = Track {
            source: PlaybackSource {
                remote_url: Some("https://example.com/stream".to_owned()),
                path: PathBuf::from("/stream"),
                ..PlaybackSource::default()
            },
            ..Track::default()
        };
        assert_eq!(
            radio_track_key(&remote),
            "https://example.com/stream".to_owned()
        );
    }

    #[test]
    fn blacklist_matches_songs_folders_and_archive_members() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        std::fs::create_dir(root.join("keep")).unwrap();
        std::fs::create_dir(root.join("drop")).unwrap();
        let song = root.join("keep/song.flac");
        let banned = root.join("drop/banned.flac");
        std::fs::write(&song, []).unwrap();
        std::fs::write(&banned, []).unwrap();
        let pack = root.join("pack.zip");
        std::fs::write(&pack, []).unwrap();
        let member = |entry: &str| {
            crate::archive::member_url(&pack, entry, false)
        };
        let rows = vec![
            ("song".to_owned(), banned.to_string_lossy().into_owned(), String::new()),
            (
                "song".to_owned(),
                pack.to_string_lossy().into_owned(),
                "inner/banned.wav".to_owned(),
            ),
            (
                "folder".to_owned(),
                root.join("drop").to_string_lossy().into_owned(),
                String::new(),
            ),
        ];
        let blacklist = Blacklist::from_rows(&rows);
        assert!(locator_is_blacklisted(&banned, &blacklist));
        assert!(!locator_is_blacklisted(&song, &blacklist));
        assert!(locator_is_blacklisted(&member("inner/banned.wav"), &blacklist));
        assert!(!locator_is_blacklisted(&member("inner/kept.wav"), &blacklist));
        assert!(Blacklist::default().songs.is_empty());
        assert!(!locator_is_blacklisted(&song, &Blacklist::default()));
    }

    #[test]
    fn probe_staging_throughput() {
        use std::time::Duration;
        let root = PathBuf::from("/mnt/stuff/Music");
        if !root.is_dir() {
            return;
        }
        let decoders = DecoderRegistry::default();
        let exts = decoders.audio_extensions();
        let cache = std::env::temp_dir().join("kog-probe-nested");
        let context = RadioCtx {
            decoders: &decoders,
            audio_exts: &exts,
            read_cue: true,
            nested_cache: &cache,
        };
        let worker = decoders.background_worker(crate::decoder::DecoderSettings::default());
        let mut round = RadioRound::new(777);
        let mut total_expand = Duration::ZERO;
        let mut total_probe = Duration::ZERO;
        let mut tracks = 0_usize;
        let mut slow = Vec::new();
        for index in 0..60 {
            let started = std::time::Instant::now();
            let Some(locator) = round.next_pick(&root, &context) else {
                break;
            };
            let descended = started.elapsed();
            let prepared = match worker.expand_detailed(locator.clone()) {
                Ok(expansion) => expansion,
                Err(error) => {
                    eprintln!("PROBE pick {index} expand ERR {error}");
                    continue;
                }
            };
            let expanded = started.elapsed();
            let mut pick_tracks = 0;
            for source in &prepared.sources {
                let probe_started = std::time::Instant::now();
                let ok = worker.probe(source).is_ok();
                let took = probe_started.elapsed();
                total_probe += took;
                if ok {
                    pick_tracks += 1;
                }
                if took > Duration::from_secs(1) {
                    slow.push((source.path.display().to_string(), took));
                }
            }
            tracks += pick_tracks;
            total_expand += expanded - descended;
            eprintln!(
                "PROBE pick {index}: descend {descended:?}, expand {:?} ({} tracks)",
                expanded - descended,
                prepared.sources.len(),
            );
        }
        eprintln!(
            "PROBE staged {tracks} tracks; expand total {total_expand:?}, probe total {total_probe:?}"
        );
        for (path, took) in &slow {
            eprintln!("PROBE SLOW {took:?} {path}");
        }
    }

    #[test]
    fn subsong_order_shuffles_uniform_sets_only() {
        let order = shuffle_subsong_order("pack.zip::a", &[0, 1, 2, 3, 4]).expect("order");
        assert_eq!(order.len(), 5);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![0, 1, 2, 3, 4], "a permutation, no loss");
        assert_eq!(
            order,
            shuffle_subsong_order("pack.zip::a", &[0, 1, 2, 3, 4]).expect("order"),
            "deterministic per file"
        );
        // Key sensitivity: across several files the orders must vary (a
        // single fixed order per key set would still pass determinism).
        // Deterministic inputs, so green stays green.
        let orders: HashSet<Vec<u32>> = ["a", "b", "c", "d", "e", "f"]
            .iter()
            .map(|key| shuffle_subsong_order(key, &[0, 1, 2, 3, 4]).expect("order"))
            .collect();
        assert!(orders.len() >= 2, "shuffles vary by file");
        assert!(shuffle_subsong_order("x", &[]).is_none());
        assert!(shuffle_subsong_order("x", &[7]).is_none());
        assert!(shuffle_subsong_order("x", &[1, 1, 2]).is_none());
        assert!(shuffle_subsong_order("x", &[0, 2]).is_some());
    }

    #[test]
    fn is_random_candidate_filters_playlists_and_optional_cue() {
        assert!(!is_random_candidate(Path::new("/music/list.m3u"), true));
        assert!(!is_random_candidate(Path::new("/music/list.m3u8"), true));
        assert!(!is_random_candidate(Path::new("/music/list.pls"), false));
        assert!(is_random_candidate(Path::new("/music/album.cue"), true));
        assert!(!is_random_candidate(Path::new("/music/album.cue"), false));
        assert!(is_random_candidate(Path::new("/music/song.flac"), true));
        assert!(is_random_candidate(Path::new("/music/pack.zip"), false));
    }

    #[test]
    fn staging_skips_known_dead_and_reports_barren() {
        use std::sync::atomic::AtomicBool;
        use std::sync::mpsc::sync_channel;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::write(root.join("live.flac"), []).unwrap();
        std::fs::write(root.join("dead.flac"), []).unwrap();
        let decoders = DecoderRegistry::default();
        let dead = Arc::new(Mutex::new(HashSet::new()));
        dead.lock()
            .unwrap()
            .insert(radio_locator_key(&root.join("dead.flac")));
        let (sender, receiver) = sync_channel(4);
        let settings = crate::decoder::DecoderSettings::default();
        let cache = root.join("cache");
        let worker_dead = Arc::clone(&dead);
        let worker_cancel = Arc::new(AtomicBool::new(false));
        let root_owned = root.to_path_buf();
        std::thread::spawn(move || {
            run_staging(
                root_owned,
                settings,
                false,
                cache,
                RoundInitial::fresh(1),
                None,
                worker_dead,
                Arc::new(Mutex::new(Blacklist::default())),
                sender,
                worker_cancel,
            );
        });
        // The dead file never arrives while the round remembers it; once the
        // round rotates, dead knowledge clears and every locator re-proves
        // itself (codecs may have appeared since). Collect enough picks to
        // cross a rotation and prove both phases.
        let mut seen_live = 0;
        let mut seen_dead = 0;
        for _ in 0..6 {
            match receiver.recv_timeout(Duration::from_secs(30)) {
                Ok(StagingResponse::Pick(pick)) => {
                    if pick == root.join("live.flac") {
                        seen_live += 1;
                    } else if pick == root.join("dead.flac") {
                        seen_dead += 1;
                    } else {
                        panic!("unexpected pick {pick:?}");
                    }
                }
                other => panic!("expected picks, got {other:?}"),
            }
        }
        assert!(seen_live >= 2, "live plays across rotations");
        assert!(seen_dead >= 1, "rotation clears dead for re-proving");
        drop(receiver);
    }

    #[test]
    fn staging_reports_barren_when_everything_is_dead() {
        use std::sync::atomic::AtomicBool;
        use std::sync::mpsc::sync_channel;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::write(root.join("dead.flac"), []).unwrap();
        let dead = Arc::new(Mutex::new(HashSet::new()));
        dead.lock()
            .unwrap()
            .insert(radio_locator_key(&root.join("dead.flac")));
        let (sender, receiver) = sync_channel(4);
        let settings = crate::decoder::DecoderSettings::default();
        let cache = root.join("cache");
        let worker_dead = Arc::clone(&dead);
        let worker_cancel = Arc::new(AtomicBool::new(false));
        let root_owned = root.to_path_buf();
        std::thread::spawn(move || {
            run_staging(
                root_owned,
                settings,
                false,
                cache,
                RoundInitial::fresh(1),
                None,
                worker_dead,
                Arc::new(Mutex::new(Blacklist::default())),
                sender,
                worker_cancel,
            );
        });
        match receiver.recv_timeout(Duration::from_secs(60)) {
            Ok(StagingResponse::Barren) => {}
            other => panic!("expected Barren, got {other:?}"),
        }
    }

    #[test]
    fn descent_exhausts_every_song_exactly_once() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::write(root.join("song.flac"), []).unwrap();
        std::fs::write(root.join("notes.txt"), []).unwrap();
        std::fs::write(root.join("list.m3u"), []).unwrap();
        std::fs::write(root.join(".hidden.flac"), []).unwrap();
        std::fs::create_dir(root.join(".sync")).unwrap();
        std::fs::write(root.join(".sync/stale.flac"), []).unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/song.mp3"), []).unwrap();
        std::fs::create_dir(root.join("empty")).unwrap();
        crate::archive::tests::write_stored_zip(
            &root.join("pack.zip"),
            &[("inner.wav", b"data"), ("cover.jpg", b"junk")],
        );
        let decoders = DecoderRegistry::default();
        let exts = test_exts();
        let context = ctx(&decoders, &exts, root);
        let mut round = RadioRound::new(99);
        let picks = drain(&mut round, root, &context);
        let member = crate::archive::member_url(&root.join("pack.zip"), "inner.wav", false);
        let mut expected = vec![root.join("song.flac"), root.join("sub/song.mp3"), member];
        let mut sorted = picks.clone();
        sorted.sort();
        expected.sort();
        assert_eq!(sorted, expected, "every playable song once, junk excluded");
        // A fresh round with the same seed replays the identical sequence.
        let mut again = RadioRound::new(99);
        assert_eq!(drain(&mut again, root, &context), picks);
    }

    #[test]
    fn descent_spreads_across_levels_first() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::create_dir_all(root.join("big/deep")).unwrap();
        std::fs::create_dir_all(root.join("big/wide")).unwrap();
        for name in ["a1", "a2", "a3", "a4", "a5"] {
            std::fs::write(root.join(format!("big/deep/{name}.flac")), []).unwrap();
        }
        for name in ["b1", "b2"] {
            std::fs::write(root.join(format!("big/wide/{name}.flac")), []).unwrap();
        }
        std::fs::create_dir(root.join("small")).unwrap();
        std::fs::write(root.join("small/lone.flac"), []).unwrap();
        let decoders = DecoderRegistry::default();
        let exts = test_exts();
        let context = ctx(&decoders, &exts, root);
        let mut round = RadioRound::new(99);
        let picks = drain(&mut round, root, &context);
        assert_eq!(picks.len(), 8);
        let top = |path: &Path| {
            path.strip_prefix(root)
                .unwrap()
                .components()
                .next()
                .unwrap()
                .as_os_str()
                .to_string_lossy()
                .into_owned()
        };
        assert_ne!(top(&picks[0]), top(&picks[1]), "first picks span tops");
        let big: Vec<bool> = picks
            .iter()
            .filter(|path| top(path) == "big")
            .map(|path| {
                path.strip_prefix(root.join("big"))
                    .unwrap()
                    .components()
                    .next()
                    .unwrap()
                    .as_os_str()
                    == "deep"
            })
            .collect();
        assert_eq!(big.len(), 7);
        assert!(
            big[0] != big[1] && big[1] != big[2] && big[2] != big[3],
            "big/ alternates deep/wide first: {big:?}"
        );
        assert!(
            big[4..].iter().all(|&deep| deep),
            "wide exhausted, deep finishes alone: {big:?}"
        );
    }

    #[test]
    fn spent_folders_replay_only_after_spacing() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/song.flac"), []).unwrap();
        let decoders = DecoderRegistry::default();
        let exts = test_exts();
        let cache = root.join("cache");
        let context = ctx(&decoders, &exts, &cache);
        let sub = fs_node(&root.join("sub"));
        // Exhausted without ever yielding: rests forever, whatever the age.
        let mut round = RadioRound::new(5);
        round.cursors.insert(
            sub.clone(),
            RadioCursor {
                done: true,
                last_yield: None,
                ..RadioCursor::default()
            },
        );
        round.counter = 1_000_000;
        assert!(round.next_pick(root, &context).is_none());
        assert!(round.cursors[&sub].done);
        // Exhausted long ago with a real yield: replays and clears.
        let mut round = RadioRound::new(5);
        round.cursors.insert(
            sub.clone(),
            RadioCursor {
                done: true,
                last_yield: Some(0),
                ..RadioCursor::default()
            },
        );
        round.counter = FOLDER_REPLAY_SPACING + 10;
        assert_eq!(
            round.next_pick(root, &context),
            Some(root.join("sub/song.flac"))
        );
        assert!(!round.cursors[&sub].done);
        // Exhausted recently: still rests.
        let mut round = RadioRound::new(5);
        round.cursors.insert(
            sub.clone(),
            RadioCursor {
                done: true,
                last_yield: Some(0),
                ..RadioCursor::default()
            },
        );
        round.counter = FOLDER_REPLAY_SPACING - 10;
        assert!(round.next_pick(root, &context).is_none());
    }

    #[test]
    fn small_folders_replay_spaced_not_looped() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::create_dir(root.join("solo")).unwrap();
        std::fs::write(root.join("solo/only.flac"), []).unwrap();
        std::fs::create_dir(root.join("bulk")).unwrap();
        for index in 0..600 {
            std::fs::write(root.join(format!("bulk/track-{index:03}.flac")), []).unwrap();
        }
        let decoders = DecoderRegistry::default();
        let exts = test_exts();
        let cache = root.join("cache");
        let context = ctx(&decoders, &exts, &cache);
        let mut round = RadioRound::new(21);
        let mut solo_at = Vec::new();
        let mut bulk_seen = HashSet::new();
        // Drain the whole round: solo replays once its first play ages out,
        // bulk exhausts exactly once each, then the round ends (~600 picks).
        // Rotation would replay bulk songs; distinctness proves it never did.
        let mut total = 0_usize;
        for index in 0..2000 {
            match round.next_pick(root, &context) {
                Some(pick) => {
                    total += 1;
                    if pick == root.join("solo/only.flac") {
                        solo_at.push(index);
                    } else {
                        assert!(
                            bulk_seen.insert(pick),
                            "bulk repeats mean an early rotation happened"
                        );
                    }
                }
                None => break,
            }
        }
        assert!(
            (600..=620).contains(&total),
            "round spans bulk plus spaced solo replays: {total}"
        );
        assert!(
            solo_at.len() >= 2,
            "solo replays within the round: {solo_at:?}"
        );
        for window in solo_at.windows(2) {
            assert!(
                window[1] - window[0] >= FOLDER_REPLAY_SPACING as usize,
                "replays spaced, never looped: {solo_at:?}"
            );
        }
    }

    #[test]
    fn tiny_libraries_rotate_cleanly_below_spacing() {
        // Libraries smaller than the replay spacing must behave as pure
        // exhaustive rounds: re-entry can never trigger (no age reaches it
        // before rotation), so every window of exactly T picks holds every
        // song exactly once, round after round, across rotations.
        use std::sync::atomic::AtomicBool;
        use std::sync::mpsc::sync_channel;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::create_dir(root.join("a")).unwrap();
        std::fs::create_dir(root.join("b")).unwrap();
        std::fs::write(root.join("a/one.flac"), []).unwrap();
        std::fs::write(root.join("a/two.flac"), []).unwrap();
        std::fs::write(root.join("b/three.flac"), []).unwrap();
        std::fs::write(root.join("b/four.flac"), []).unwrap();
        std::fs::write(root.join("top.flac"), []).unwrap();
        let mut expected: Vec<PathBuf> = ["a/one", "a/two", "b/three", "b/four", "top"]
            .iter()
            .map(|stem| root.join(format!("{stem}.flac")))
            .collect();
        expected.sort();
        let (sender, receiver) = sync_channel(4);
        let settings = crate::decoder::DecoderSettings::default();
        let cache = root.join("cache");
        let dead = Arc::new(Mutex::new(HashSet::new()));
        let worker_dead = Arc::clone(&dead);
        let worker_cancel = Arc::new(AtomicBool::new(false));
        let root_owned = root.to_path_buf();
        std::thread::spawn(move || {
            run_staging(
                root_owned,
                settings,
                false,
                cache,
                RoundInitial::fresh(77),
                None,
                worker_dead,
                Arc::new(Mutex::new(Blacklist::default())),
                sender,
                worker_cancel,
            );
        });
        for _ in 0..3 {
            let mut window = Vec::new();
            for _ in 0..5 {
                match receiver.recv_timeout(Duration::from_secs(30)) {
                    Ok(StagingResponse::Pick(pick)) => window.push(pick),
                    other => panic!("expected picks, got {other:?}"),
                }
            }
            window.sort();
            assert_eq!(window, expected, "each round plays every song once");
        }
    }

    #[test]
    fn nested_archives_descend_fully() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        let scratch = tempfile::tempdir().unwrap();
        let inner = scratch.path().join("inner.zip");
        crate::archive::tests::write_stored_zip(&inner, &[("song.wav", b"data")]);
        let inner_bytes = std::fs::read(&inner).unwrap();
        crate::archive::tests::write_stored_zip(
            &root.join("outer.zip"),
            &[
                ("inner.zip", inner_bytes.as_slice()),
                ("top.wav", b"data".as_slice()),
            ],
        );
        let decoders = DecoderRegistry::default();
        let exts = test_exts();
        let cache_holder = tempfile::tempdir().unwrap();
        let cache = cache_holder.path().join("cache");
        let context = ctx(&decoders, &exts, &cache);
        let mut round = RadioRound::new(7);
        let picks = drain(&mut round, root, &context);
        let outer = root.join("outer.zip");
        let mut expected = vec![
            crate::archive::member_url(&outer, "top.wav", false),
            crate::archive::member_url(&outer, "inner.zip/song.wav", false),
        ];
        let mut sorted = picks.clone();
        sorted.sort();
        expected.sort();
        assert_eq!(sorted, expected, "nested members reachable as picks");
        for pick in &picks {
            crate::archive::tree_location(pick)
                .expect("parse member")
                .expect("location");
        }
    }

    #[test]
    fn over_nested_archives_are_skipped_safely() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        // Five-deep nesting exceeds the 4-level safety cap; the innermost
        // song must never surface, but the round still terminates.
        let mut inner = root.join("deepest.wav");
        std::fs::write(&inner, b"data").unwrap();
        for level in (0..5).rev() {
            let outer = root.join(format!("level{level}.zip"));
            let bytes = std::fs::read(&inner).unwrap();
            let name = inner.file_name().unwrap().to_string_lossy().into_owned();
            crate::archive::tests::write_stored_zip(&outer, &[(name.as_str(), bytes.as_slice())]);
            inner = outer;
        }
        std::fs::write(root.join("plain.flac"), []).unwrap();
        let decoders = DecoderRegistry::default();
        let exts = test_exts();
        let cache_holder = tempfile::tempdir().unwrap();
        let cache = cache_holder.path().join("cache");
        let context = ctx(&decoders, &exts, &cache);
        let mut round = RadioRound::new(3);
        let picks = drain(&mut round, root, &context);
        // Every chain of at most 4 links stays reachable through its
        // shallowest entry point; only the 5-link copy through level0.zip
        // stays out while the round still terminates.
        let mut expected = vec![root.join("plain.flac"), root.join("deepest.wav")];
        for start in 1..=4 {
            let chain: Vec<String> = ((start + 1)..=4)
                .map(|level| format!("level{level}.zip"))
                .chain(std::iter::once("deepest.wav".to_owned()))
                .collect();
            expected.push(crate::archive::member_url(
                &root.join(format!("level{start}.zip")),
                &chain.join("/"),
                false,
            ));
        }
        let mut sorted = picks.clone();
        sorted.sort();
        expected.sort();
        assert_eq!(sorted, expected);
    }
}
