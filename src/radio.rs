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

pub(crate) fn random_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|age| age.as_secs() ^ u64::from(age.subsec_nanos()))
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
}

/// Salt distinguishing file order from turn order inside one folder.
const FILE_ORDER_SALT: u64 = 0x9E37_79B9_7F4A_7C15;

/// Picks produced by the staging thread: locators in round order, Empty
/// when the library yielded nothing across two consecutive rounds, Barren
/// when everything reachable already failed this session.
#[derive(Debug)]
pub(crate) enum StagingResponse {
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
pub(crate) fn run_staging(
    root: PathBuf,
    settings: DecoderSettings,
    read_cue: bool,
    nested_cache: PathBuf,
    dead: std::sync::Arc<std::sync::Mutex<HashSet<String>>>,
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
    let mut round = RadioRound::new(random_seed());
    // Consecutive rounds with zero yields of any kind end the thread; a
    // round that yields (live or skipped) proves content and resets both.
    let mut empty_rounds = 0_usize;
    let mut round_yielded = false;
    let mut skips = 0_usize;
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        match round.next_pick(&root, &ctx) {
            Some(locator) => {
                round_yielded = true;
                let known_dead = dead
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .contains(&radio_locator_key(&locator));
                if known_dead {
                    skips += 1;
                    if skips >= MAX_SKIPS {
                        let _ = picks.send(StagingResponse::Barren);
                        return;
                    }
                    continue;
                }
                skips = 0;
                if picks.send(StagingResponse::Pick(locator)).is_err() {
                    return;
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
                    return;
                }
                round = RadioRound::new(random_seed());
                round_yielded = false;
            }
        }
    }
}
/// Per-folder radio state: positions in the functionally-shuffled orders.
/// That is the only thing a round remembers; orders are recomputed.
#[derive(Default)]
struct RadioCursor {
    entry: usize,
    file: usize,
    files_done: bool,
    done: bool,
}

/// One round of exhaustive picks: a seed plus a cursor per visited folder.
pub(crate) struct RadioRound {
    seed: u64,
    cursors: HashMap<String, RadioCursor>,
}

/// Everything descent needs that is not the round itself. Built once per
/// refill so member filtering does not rebuild the extension set per folder.
pub(crate) struct RadioCtx<'a> {
    pub decoders: &'a DecoderRegistry,
    pub audio_exts: &'a HashSet<String>,
    pub read_cue: bool,
    pub nested_cache: &'a Path,
}

impl RadioRound {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            seed,
            cursors: HashMap::new(),
        }
    }

    /// Next pick from the music root: descend level by level, or None when
    /// every song has played once and the round is exhausted.
    pub(crate) fn next_pick(&mut self, root: &Path, ctx: &RadioCtx) -> Option<PathBuf> {
        self.descend(&fs_node(root), ctx)
    }

    fn node_seed(&self, node: &str) -> u64 {
        hash_with_seed(self.seed, node.as_bytes())
    }

    /// Serve one pick from `node`: cycle its turns in shuffled order,
    /// skipping turns that already exhausted themselves this round.
    fn descend(&mut self, node: &str, ctx: &RadioCtx) -> Option<PathBuf> {
        if self.cursors.get(node).is_some_and(|cursor| cursor.done) {
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
                    return Some(locator);
                }
                cursor.files_done = true;
            } else {
                let child = listing.dirs[turn - 1].1.clone();
                if self.cursors.get(&child).is_some_and(|cursor| cursor.done) {
                    continue;
                }
                if let Some(locator) = self.descend(&child, ctx) {
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
pub(crate) fn radio_track_key(track: &crate::track::Track) -> String {
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
pub(crate) fn radio_locator_key(path: &Path) -> String {
    if let Ok(Some(location)) = crate::archive::tree_location(path) {
        return format!("{}::{}", location.archive.display(), location.entry);
    }
    path.display().to_string()
}

/// Whether a playlist/archive path is eligible for random queueing: regular
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
        if crate::media_path::is_metadata(&path) {
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
        if child.starts_with('.') || crate::media_path::is_metadata(Path::new(&full)) {
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

#[cfg(test)]
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
                worker_dead,
                sender,
                worker_cancel,
            );
        });
        // The dead file never arrives; live rounds keep cycling instead.
        for _ in 0..4 {
            match receiver.recv_timeout(Duration::from_secs(30)) {
                Ok(StagingResponse::Pick(pick)) => assert_eq!(pick, root.join("live.flac")),
                other => panic!("expected the live pick, got {other:?}"),
            }
        }
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
                worker_dead,
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
