//! Album cover resolution: embedded tags, then a disk cache, then download
//! providers (Deezer, iTunes, MusicBrainz/Cover Art Archive, DuckDuckGo).
//! Network requests stay in each frontend's worker. Provider order, matching,
//! image validation, and MusicBrainz throttling are shared here.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const MAX_COVER_BYTES: u32 = 8 * 1024 * 1024;
/// A platform's box-art listing runs to a few megabytes.
const MAX_BOX_ART_INDEX_BYTES: u32 = 8 * 1024 * 1024;
/// Box-art guesses asked at once.
const BOX_ART_BATCH: usize = 12;
pub const MAX_SEARCH_BYTES: u32 = 512 * 1024;
/// The cover cache's budget. The server's playlist-cover worker downloads a
/// cover for every playlist's songs (a box-art PNG runs to most of a
/// megabyte, and each album keeps its own copy beside the game's), so the
/// budget holds a large library's: at 200 MB, the newest downloads evicted
/// the older ones before anyone saw them.
pub const MAX_CACHE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const CACHE_SUBDIRECTORY: &str = "covers";

fn musicbrainz_rate_limit() {
    static LAST_REQUEST: OnceLock<Mutex<Instant>> = OnceLock::new();
    let guard = LAST_REQUEST.get_or_init(|| Mutex::new(Instant::now() - Duration::from_secs(2)));
    if let Ok(mut last) = guard.lock() {
        let elapsed = last.elapsed();
        if elapsed < Duration::from_millis(1100) {
            std::thread::sleep(Duration::from_millis(1100) - elapsed);
        }
        *last = Instant::now();
    }
}

/// Search and validate a cover with the same provider policy in native UIs.
/// The caller supplies its network transport and cache/preview adapter. A
/// rejected or unstorable candidate falls through to the next provider.
///
/// `file` is the song's own file: game music (an NSF, SPC, PSF, a MIDI from a
/// game folder, ...) first tries the game's box art from the libretro
/// thumbnail server, named by the album, for the consoles its format comes
/// from.
pub fn download_cover<T>(
    file: &Path,
    artist: &str,
    album: &str,
    fetch: impl Fn(&str, u32) -> Option<Vec<u8>> + Sync,
    cancelled: impl Fn() -> bool,
    mut store: impl FnMut(Vec<u8>) -> Option<T>,
) -> Option<T> {
    macro_rules! fetch_image {
        ($url:expr) => {{
            fetch(&$url, MAX_COVER_BYTES)
                .filter(|bytes| sniff_image_kind(bytes).is_some())
                .and_then(&mut store)
        }};
    }
    if cancelled() {
        return None;
    }
    // Most guessed box-art names miss, and each miss is a round trip, so a
    // batch of them is asked at once; the earliest name that hits wins.
    for batch in game_box_art_urls(album, &game_platforms(file)).chunks(BOX_ART_BATCH) {
        if cancelled() {
            return None;
        }
        let found = std::thread::scope(|scope| {
            let probes: Vec<_> = batch
                .iter()
                .map(|url| {
                    let fetch = &fetch;
                    scope.spawn(move || {
                        fetch(url, MAX_COVER_BYTES).filter(|bytes| sniff_image_kind(bytes).is_some())
                    })
                })
                .collect();
            probes
                .into_iter()
                .find_map(|probe| probe.join().ok().flatten())
        });
        if let Some(result) = found.and_then(&mut store) {
            return Some(result);
        }
    }
    // No guessed name exists: look the game up in the platform's listing,
    // where names carry tags such as "(1986)(Nintendo)(VS UniSystem)".
    let game = !game_platforms(file).is_empty();
    for platform in game_platforms(file) {
        if cancelled() {
            return None;
        }
        // A few of the best matches, so a rejected box gives way to the next.
        if let Some(names) = box_art_index(platform, &fetch) {
            for name in box_art_matches(album, &names).into_iter().take(4) {
                if cancelled() {
                    return None;
                }
                if let Some(result) = fetch_image!(box_art_url(platform, name)) {
                    return Some(result);
                }
            }
        }
    }
    // Game music wants the game's box art, not a music album that happens
    // to share its name ("Crystalis" finds a band's album on Deezer): an
    // image search for the box art comes before the music catalogues.
    if game {
        for image in image_search_images(file, artist, album, &fetch) {
            if cancelled() {
                return None;
            }
            if let Some(result) = fetch_image!(image) {
                return Some(result);
            }
        }
    }
    if cancelled() {
        return None;
    }
    if let Some(json) = fetch(&deezer_search_url(artist, album), MAX_SEARCH_BYTES)
        && let Ok(text) = String::from_utf8(json)
        && let Some((title, match_artist, cover)) = parse_deezer_cover(&text)
        && !cover.is_empty()
        && titles_match(artist, album, &title, &match_artist)
        && let Some(result) = fetch_image!(cover)
    {
        return Some(result);
    }
    if cancelled() {
        return None;
    }
    if let Some(json) = fetch(&itunes_search_url(artist, album), MAX_SEARCH_BYTES)
        && let Ok(text) = String::from_utf8(json)
        && let Some((title, match_artist, cover)) = parse_itunes_cover(&text)
        && !cover.is_empty()
        && titles_match(artist, album, &title, &match_artist)
        && let Some(result) = fetch_image!(cover)
    {
        return Some(result);
    }
    let lookup_album = album_lookup_name(album);
    if cancelled() {
        return None;
    }
    musicbrainz_rate_limit();
    if cancelled() {
        return None;
    }
    if let Some(json) = fetch(
        &mb_release_group_url(artist, lookup_album),
        MAX_SEARCH_BYTES,
    ) && let Ok(text) = String::from_utf8(json)
    {
        for (title, match_artist, mbid) in parse_mb_release_groups(&text) {
            if cancelled() {
                return None;
            }
            if !mbid.is_empty()
                && titles_match(artist, lookup_album, &title, &match_artist)
                && let Some(result) = fetch_image!(caa_front_url(&mbid))
            {
                return Some(result);
            }
        }
    }
    let artist_queries = if artist.trim().is_empty() {
        vec![""]
    } else {
        vec![artist, ""]
    };
    for query_artist in artist_queries {
        if cancelled() {
            return None;
        }
        musicbrainz_rate_limit();
        if cancelled() {
            return None;
        }
        if let Some(json) = fetch(
            &mb_release_url(query_artist, lookup_album),
            MAX_SEARCH_BYTES,
        ) && let Ok(text) = String::from_utf8(json)
        {
            let releases = parse_mb_releases(&text);
            if query_artist.is_empty() && !consistent_exact_releases(&releases, lookup_album) {
                continue;
            }
            for (title, match_artist, mbid) in releases {
                if cancelled() {
                    return None;
                }
                let matches = if query_artist.is_empty() {
                    album_title_exact(lookup_album, &title)
                } else {
                    titles_match(query_artist, lookup_album, &title, &match_artist)
                };
                if matches
                    && !mbid.is_empty()
                    && let Some(result) = fetch_image!(caa_release_front_url(&mbid))
                {
                    return Some(result);
                }
            }
        }
    }
    if cancelled() {
        return None;
    }
    // Last resort for an album, an image search, so the cover shows
    // something. (A game's was tried before the music catalogues.)
    if !game {
        for image in image_search_images(file, artist, album, &fetch) {
            if cancelled() {
                return None;
            }
            if let Some(result) = fetch_image!(image) {
                return Some(result);
            }
        }
    }
    None
}

/// Image-search results worth fetching for a cover: the first results whose
/// title names the album (or game), then the top results. A game is searched
/// as its box art on its console.
fn image_search_images(
    file: &Path,
    artist: &str,
    album: &str,
    fetch: &impl Fn(&str, u32) -> Option<Vec<u8>>,
) -> Vec<String> {
    let query = image_search_query(file, artist, album);
    let Some(page) = fetch(&ddg_page_url(&query), MAX_SEARCH_BYTES) else {
        return Vec::new();
    };
    let Some(token) = String::from_utf8(page).ok().and_then(|html| ddg_token(&html)) else {
        return Vec::new();
    };
    let Some(text) = fetch(&ddg_image_url(&query, &token), MAX_SEARCH_BYTES)
        .and_then(|results| String::from_utf8(results).ok())
    else {
        return Vec::new();
    };
    let images = parse_ddg_images(&text);
    let game = !game_platforms(file).is_empty();
    let lookup = search_title(file, album);
    let matching = images
        .iter()
        .filter(|(title, _)| titles_match(if game { "" } else { artist }, &lookup, title, ""))
        .take(2);
    let mut urls: Vec<String> = Vec::new();
    for (_, image) in matching.chain(images.iter().take(2)) {
        if !urls.contains(image) {
            urls.push(image.clone());
        }
    }
    urls
}

/// The title a search asks for: a game's plain title, without the release
/// tags and alternate names rips carry ("Crystalis [God Slayer - Haruka
/// Tenkuu no Sonata] (1990-04-13)(SNK)" is "Crystalis"), or the album.
fn search_title(file: &Path, album: &str) -> String {
    if game_platforms(file).is_empty() {
        album_lookup_name(album).to_owned()
    } else {
        without_tag_groups(album_lookup_name(album))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Jpeg,
    Png,
}

impl ImageKind {
    pub fn extension(self) -> &'static str {
        match self {
            ImageKind::Jpeg => "jpg",
            ImageKind::Png => "png",
        }
    }
}

/// JPEG/PNG magic bytes only: never cache an HTML error page as artwork.
pub fn sniff_image_kind(bytes: &[u8]) -> Option<ImageKind> {
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        Some(ImageKind::Jpeg)
    } else if bytes.len() >= 8 && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        Some(ImageKind::Png)
    } else {
        None
    }
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn sanitize_component(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | '.' | ' ') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    cleaned.trim().trim_matches('.').to_owned()
}

/// Deterministic cache key readable enough to debug: a sanitized
/// "artist - album" stem plus a hash so near-identical names cannot collide.
pub fn cache_key(artist: &str, album: &str) -> String {
    let mut fingerprint = String::with_capacity(artist.len() + album.len() + 2);
    fingerprint.push_str(artist.trim());
    fingerprint.push('\0');
    fingerprint.push_str(album.trim());
    fingerprint.push('\0');
    let hash = fnv1a64(fingerprint.as_bytes());
    let mut stem = sanitize_component(&format!("{} - {}", artist.trim(), album.trim()));
    if stem.is_empty() {
        stem.push_str("untitled");
    }
    let truncated: String = stem.chars().take(60).collect();
    format!("{truncated}-{hash:016x}")
}

/// Give wholly untagged files their own embedded-art cache entry and avoid
/// downloading one guessed folder cover for every file in that folder.
pub fn track_cache_key(
    artist: &str,
    tagged_album: &str,
    lookup_album: &str,
    file: &Path,
) -> (String, bool) {
    if artist.trim().is_empty() && tagged_album.trim().is_empty() {
        let scoped = format!("{} \0 {}", lookup_album, file.display());
        (cache_key("", &scoped), false)
    } else if !game_platforms(file).is_empty() {
        // Game music keeps its own entries: its cover is the game's box art,
        // not whatever album a music search matched before box art was tried.
        // One per game: rips often credit a different composer per track.
        // "v2": covers found before a game's image search came ahead of the
        // music catalogues may be a music album's, so they are looked up again.
        (cache_key("", &format!("game-v2 \0 {lookup_album}")), true)
    } else {
        (cache_key(artist, lookup_album), true)
    }
}

/// The key [`track_cache_key`] gave game music before its "v2": the cover
/// found then (perhaps a music album's) still stands in, while the box-art
/// lookup finds the game's own, rather than showing none at all.
pub fn previous_track_cache_key(
    artist: &str,
    tagged_album: &str,
    lookup_album: &str,
    file: &Path,
) -> Option<String> {
    let untagged = artist.trim().is_empty() && tagged_album.trim().is_empty();
    (!untagged && !game_platforms(file).is_empty())
        .then(|| cache_key("", &format!("game \0 {lookup_album}")))
}

pub fn cache_directory(base: &Path) -> PathBuf {
    base.join(CACHE_SUBDIRECTORY)
}

/// Directory names that commonly hold scanned artwork next to music.
const ART_SUBDIRECTORIES: [&str; 5] = ["covers", "cover", "artwork", "scans", "scan"];
/// File stems preferred as the primary artwork, most specific first.
const PREFERRED_ART_STEMS: [&str; 6] = [
    "front",
    "frontcover",
    "cover",
    "folder",
    "album",
    "coverart",
];
/// Substrings marking booklet/back/disc scans: never promoted when any
/// other image exists, so a back cover never becomes the artwork.
const REJECTED_ART_SUBSTRINGS: [&str; 10] = [
    "back", "rear", "disc", "tray", "inside", "inlay", "spine", "label", "cd", "dvd",
];

/// Best sibling artwork for a music file: exact front/cover/folder-style
/// names beside the file, then in well-known art subdirectories, then any
/// other image there that is not obviously a back/disc scan. Rips that
/// keep scans in a `covers/` folder (with untagged `01.flac`-style files
/// and no embeddable art) resolve through the last two tiers.
pub fn sibling_cover_bytes(file_path: &Path) -> Option<Vec<u8>> {
    if !file_path.is_file() {
        return None;
    }
    let dir = file_path.parent()?;
    let mut subdirs = vec![dir.to_path_buf()];
    for name in ART_SUBDIRECTORIES {
        let sub = dir.join(name);
        if sub.is_dir() {
            subdirs.push(sub);
        }
    }
    // Tiers 1+2: exact preferred stems, track directory first.
    for sub in &subdirs {
        let mut ranked: Vec<(usize, PathBuf)> = art_file_candidates(sub)
            .into_iter()
            .filter_map(|candidate| {
                PREFERRED_ART_STEMS
                    .iter()
                    .position(|keep| *keep == stem_lowercase(&candidate))
                    .map(|rank| (rank, candidate))
            })
            .collect();
        ranked.sort();
        for (_, path) in ranked {
            if let Some(bytes) = read_valid_art(&path) {
                return Some(bytes);
            }
        }
    }
    // Tier 3: anything else in the art subdirectories except
    // back/disc-style scans.
    for sub in subdirs.iter().skip(1) {
        for candidate in art_file_candidates(sub) {
            let stem = stem_lowercase(&candidate);
            if REJECTED_ART_SUBSTRINGS
                .iter()
                .any(|reject| stem.contains(reject))
            {
                continue;
            }
            if let Some(bytes) = read_valid_art(&candidate) {
                return Some(bytes);
            }
        }
    }
    None
}

/// Folder-scoped cache key so sibling art is shared across the folder
/// without leaking into other folders' files.
pub fn folder_cover_key(folder: &Path) -> String {
    cache_key("folder", &folder.to_string_lossy())
}

fn art_file_candidates(dir: &Path) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return entries;
    };
    for entry in read.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let image = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                let extension = extension.to_ascii_lowercase();
                extension == "jpg" || extension == "jpeg" || extension == "png"
            });
        if image {
            entries.push(path);
        }
    }
    entries.sort();
    entries
}

fn stem_lowercase(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn read_valid_art(path: &Path) -> Option<Vec<u8>> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() == 0 || metadata.len() > u64::from(MAX_COVER_BYTES) {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if sniff_image_kind(&bytes).is_some() {
        Some(bytes)
    } else {
        None
    }
}

/// A previously cached cover for this key, if any.
pub fn cache_lookup(cache_dir: &Path, key: &str) -> Option<PathBuf> {
    for extension in ["jpg", "png"] {
        let path = cache_dir.join(format!("{key}.{extension}"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// How long a search that found no cover is trusted before trying again.
pub const MISS_RETRY: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);

fn miss_path(cache_dir: &Path, key: &str) -> PathBuf {
    cache_dir.join(format!("{key}.miss"))
}

/// Remember that downloading found no cover, so replaying the album does not
/// search every provider again.
pub fn record_miss(cache_dir: &Path, key: &str) {
    if std::fs::create_dir_all(cache_dir).is_ok() {
        let _ = std::fs::write(miss_path(cache_dir, key), b"");
    }
}

/// True when a download for `key` found nothing within `MISS_RETRY`.
pub fn recent_miss(cache_dir: &Path, key: &str) -> bool {
    std::fs::metadata(miss_path(cache_dir, key))
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age < MISS_RETRY)
}

/// Forget a remembered miss, so the next play searches again.
pub fn clear_miss(cache_dir: &Path, key: &str) {
    let _ = std::fs::remove_file(miss_path(cache_dir, key));
}

/// Hashes of covers the listener marked wrong, one per line. Kept out of
/// eviction: a rejected cover must stay rejected.
const REJECTED_COVERS: &str = "rejected-covers.txt";
/// The covers changed by hand, one line each: the replaced image's hash, or
/// `-` for an album that had none. The desktop app and the servers sharing
/// the cache (one may run in a container) remember answers in memory and
/// keep small copies; they follow this log to forget them.
const COVER_CHANGES: &str = "cover-changes.txt";

fn image_hash(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a64(bytes))
}

/// True when the listener marked these exact image bytes as a wrong cover.
pub fn is_rejected(cache_dir: &Path, bytes: &[u8]) -> bool {
    let hash = image_hash(bytes);
    std::fs::read_to_string(cache_dir.join(REJECTED_COVERS))
        .is_ok_and(|list| list.lines().any(|line| line == hash))
}

/// Mark a cached cover as wrong: it is deleted and never stored again, so
/// the next lookup moves on to the next source. Works for downloaded,
/// embedded and folder art alike.
pub fn reject_cached_cover(cache_dir: &Path, path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    reject_cover(cache_dir, &bytes) && std::fs::remove_file(path).is_ok()
}

/// Mark these image bytes as a wrong cover and delete every cached copy.
pub fn reject_cover(cache_dir: &Path, bytes: &[u8]) -> bool {
    use std::io::Write;
    let hash = image_hash(bytes);
    if std::fs::create_dir_all(cache_dir).is_err() {
        return false;
    }
    let appended = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cache_dir.join(REJECTED_COVERS))
        .and_then(|mut list| writeln!(list, "{hash}"));
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !is_cache_list(&entry.file_name())
                && std::fs::read(&path).is_ok_and(|cached| cached == bytes)
            {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    note_cover_change(cache_dir, Some(bytes));
    appended.is_ok()
}

fn is_cache_list(name: &std::ffi::OsStr) -> bool {
    name == REJECTED_COVERS || name == COVER_CHANGES
}

/// A cover was changed by hand (chosen or rejected): `replaced` is the image
/// no longer shown, if there was one. Every program sharing the cache
/// forgets what it remembered (see [`cover_changes_since`]).
pub fn note_cover_change(cache_dir: &Path, replaced: Option<&[u8]>) {
    use std::io::Write;
    if std::fs::create_dir_all(cache_dir).is_err() {
        return;
    }
    let line = replaced.map(image_hash).unwrap_or_else(|| "-".to_owned());
    // One short append: concurrent writers never interleave within a line.
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cache_dir.join(COVER_CHANGES))
        .and_then(|mut log| log.write_all(format!("{line}\n").as_bytes()));
}

/// How far the cover-change log has grown: a later answer that differs means
/// covers were changed by hand meanwhile.
pub fn cover_changes_stamp(cache_dir: &Path) -> u64 {
    std::fs::metadata(cache_dir.join(COVER_CHANGES))
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

/// The covers changed by hand since `stamp` (from [`cover_changes_stamp`] or
/// an earlier call): the new stamp, and the hashes of the images replaced
/// (`None` for an album that had none). A log that shrank is read whole.
pub fn cover_changes_since(cache_dir: &Path, stamp: u64) -> (u64, Vec<Option<u64>>) {
    let Ok(bytes) = std::fs::read(cache_dir.join(COVER_CHANGES)) else {
        return (0, Vec::new());
    };
    let length = bytes.len() as u64;
    let start = if stamp <= length { stamp as usize } else { 0 };
    // Only whole lines: a writer may be halfway through the last one.
    let Some(end) = bytes[start..].iter().rposition(|byte| *byte == b'\n') else {
        return (start as u64, Vec::new());
    };
    let changes = String::from_utf8_lossy(&bytes[start..start + end])
        .lines()
        .map(|line| u64::from_str_radix(line.trim(), 16).ok())
        .collect();
    ((start + end + 1) as u64, changes)
}

/// Validate and persist downloaded or embedded bytes. Runs cache eviction
/// first so the budget holds even on a full disk of covers. A cover the
/// listener rejected is refused, so a download goes on to the next source.
pub fn store_cache(cache_dir: &Path, key: &str, bytes: &[u8]) -> Option<PathBuf> {
    let kind = sniff_image_kind(bytes)?;
    if is_rejected(cache_dir, bytes) {
        return None;
    }
    if std::fs::create_dir_all(cache_dir).is_err() {
        return None;
    }
    evict_cache(cache_dir, MAX_CACHE_BYTES);
    let path = cache_dir.join(format!("{key}.{}", kind.extension()));
    // Through a uniquely named temporary file and a rename: the desktop app
    // and the server (also from a container) share this folder, and a reader
    // must never see a half-written cover.
    let temporary = cache_dir.join(format!(
        ".{key}.{}.{:x}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ));
    if std::fs::write(&temporary, bytes).is_err() || std::fs::rename(&temporary, &path).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return None;
    }
    Some(path)
}

/// Delete the least-recently-modified covers until the directory is back
/// under budget. Best effort: every failure just stops the sweep.
pub fn evict_cache(cache_dir: &Path, max_bytes: u64) {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return;
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() || is_cache_list(&entry.file_name()) {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((modified, metadata.len(), entry.path()))
        })
        .collect::<Vec<_>>();
    let mut total: u64 = files.iter().map(|(_, size, _)| *size).sum();
    if total <= max_bytes {
        return;
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, size, path) in files {
        if total <= max_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}

fn encode(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}

fn query_text(artist: &str, album: &str) -> String {
    format!("{} {}", artist.trim(), album.trim())
        .trim()
        .to_owned()
}

/// libretro's thumbnail server: box art named after each game's No-Intro
/// (or similar) title, per platform. No key needed.
const LIBRETRO_THUMBNAILS: &str = "https://thumbnails.libretro.com";

/// The libretro platforms a game-music file's format belongs to, most likely
/// first. Formats that are not game music, and MIDI files outside a game
/// folder, give none.
pub fn game_platforms(file: &Path) -> Vec<&'static str> {
    let extension = file
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "nsf" | "nsfe" => vec!["Nintendo - Nintendo Entertainment System"],
        "spc" | "rsn" => vec!["Nintendo - Super Nintendo Entertainment System"],
        "gbs" => vec!["Nintendo - Game Boy", "Nintendo - Game Boy Color"],
        "gsf" | "minigsf" => vec!["Nintendo - Game Boy Advance"],
        "2sf" | "mini2sf" | "ncsf" | "minincsf" => vec!["Nintendo - Nintendo DS"],
        "usf" | "miniusf" => vec!["Nintendo - Nintendo 64"],
        "psf" | "minipsf" => vec!["Sony - PlayStation"],
        "psf2" | "minipsf2" => vec!["Sony - PlayStation 2"],
        "ssf" | "minissf" => vec!["Sega - Saturn"],
        "dsf" | "minidsf" => vec!["Sega - Dreamcast"],
        "hes" => vec!["NEC - PC Engine - TurboGrafx 16"],
        "sid" | "psid" | "rsid" => vec!["Commodore - 64"],
        "kss" => vec!["Microsoft - MSX", "Microsoft - MSX2"],
        // A VGM can hold any Sega 8- or 16-bit console's music: all of
        // them, the Mega Drive first, never guessed from folder names.
        "vgm" | "vgz" => vec![
            "Sega - Mega Drive - Genesis",
            "Sega - Master System - Mark III",
            "Sega - Game Gear",
        ],
        _ => Vec::new(),
    }
}


/// Box-art URLs to try for a game on its platforms: its title as tagged, with
/// any trailing "(1986-08)(Nintendo)" groups dropped, a leading "The" moved
/// to the end, and the usual region suffixes.
pub fn game_box_art_urls(game: &str, platforms: &[&str]) -> Vec<String> {
    if platforms.is_empty() {
        return Vec::new();
    }
    let base = without_tag_groups(album_lookup_name(game));
    if base.is_empty() {
        return Vec::new();
    }
    let mut titles = vec![base.clone()];
    if let Some(rest) = base.strip_prefix("The ") {
        titles.push(format!("{rest}, The"));
    }
    if base.contains(": ") {
        titles.push(base.replace(": ", " - "));
    }
    let mut urls = Vec::new();
    for platform in platforms {
        for title in &titles {
            for region in ["", " (USA)", " (World)", " (USA, Europe)", " (Europe)", " (Japan)"] {
                // libretro's file names stand `&*/:`<>?\|"` in as `_`.
                let name: String = format!("{title}{region}")
                    .chars()
                    .map(|c| if "&*/:`<>?\\|\"".contains(c) { '_' } else { c })
                    .collect();
                urls.push(format!(
                    "{LIBRETRO_THUMBNAILS}/{}/Named_Boxarts/{}.png",
                    path_encode(platform),
                    path_encode(&name)
                ));
            }
        }
    }
    urls
}

/// A game title without trailing tag groups:
/// "Metroid (1986-08)(Nintendo R&D1)(Nintendo)" is "Metroid".
fn without_tag_groups(title: &str) -> String {
    let mut base = title.trim().to_owned();
    while let Some(open) = base.rfind(['(', '[']) {
        if open == 0 || !(base.ends_with(')') || base.ends_with(']')) {
            break;
        }
        base = base[..open].trim_end().to_owned();
    }
    base
}

/// A title's words for comparing names loosely: case, punctuation, tag
/// groups and a leading or trailing "The" ignored, "&" read as "and", and
/// Roman numerals read as numbers. "VS. Super Mario Bros" and
/// "Vs. Super Mario Bros. (1986)(Nintendo)" both give "vs super mario bros";
/// "Ultima V" and "Ultima 5" both give "ultima 5".
pub fn game_title_words(title: &str) -> String {
    const ROMAN: [&str; 20] = [
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii", "xiii", "xiv",
        "xv", "xvi", "xvii", "xviii", "xix", "xx",
    ];
    const NUMBERS: [&str; 20] = [
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
        "17", "18", "19", "20",
    ];
    let base = without_tag_groups(title).to_lowercase().replace('&', " and ");
    let mut words: Vec<&str> = base
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            ROMAN
                .iter()
                .position(|numeral| *numeral == word)
                .map_or(word, |index| NUMBERS[index])
        })
        .collect();
    if words.first() == Some(&"the") {
        words.remove(0);
    }
    if words.last() == Some(&"the") {
        words.pop();
    }
    words.join(" ")
}

/// The listing of a platform's box art on the libretro thumbnail server.
pub fn box_art_index_url(platform: &str) -> String {
    format!("{LIBRETRO_THUMBNAILS}/{}/Named_Boxarts/?F=0", path_encode(platform))
}

/// Image names (without ".png") in a libretro directory listing.
pub fn parse_box_art_index(html: &str) -> Vec<String> {
    html.split("href=\"")
        .skip(1)
        .filter_map(|rest| {
            let name = rest.split('"').next()?.strip_suffix(".png")?;
            (!name.contains('/')).then(|| {
                percent_encoding::percent_decode_str(name)
                    .decode_utf8_lossy()
                    .into_owned()
            })
        })
        .collect()
}

/// The listed box art for `game`, best first: titles that match loosely,
/// then titles that match once the subtitle is dropped, then titles sharing
/// most of their words, so a game's music shows something close (a sequel's
/// or another release's box) rather than nothing. Among equals a good dump
/// comes first, then a USA or World release, then the lowest part number
/// ("Part 1" before "Part 2"), then the shortest name.
pub fn box_art_matches<'a>(game: &str, names: &'a [String]) -> Vec<&'a str> {
    let wanted = game_title_words(album_lookup_name(game));
    let wanted_words: Vec<&str> = wanted.split(' ').filter(|word| !word.is_empty()).collect();
    if wanted_words.is_empty() {
        return Vec::new();
    }
    let rank = |name: &String| {
        let lower = name.to_lowercase();
        let flawed = ["[b", "[h", "(beta", "(proto", "(hack", "bootleg"]
            .iter()
            .any(|mark| lower.contains(mark));
        let preferred = lower.contains("(usa") || lower.contains("(world");
        let part = game_title_words(name)
            .split(' ')
            .skip(wanted_words.len())
            .find_map(|word| word.parse::<u32>().ok())
            .unwrap_or(0);
        (flawed, !preferred, part, name.len())
    };
    let mut exact: Vec<&String> = names.iter().filter(|name| game_title_words(name) == wanted).collect();
    exact.sort_by_key(|name| rank(name));
    // The game's title without its subtitle: "Ultima 5" for "Ultima V -
    // Warriors of Destiny" (libretro writes a title's ":" as "_").
    let mut main_title: Vec<&String> = names
        .iter()
        .filter(|name| {
            let title = without_tag_groups(name);
            let main = title.split([':', '_']).next().unwrap_or_default();
            let main = main.split(" - ").next().unwrap_or_default();
            game_title_words(main) == wanted && game_title_words(name) != wanted
        })
        .collect();
    main_title.sort_by_key(|name| rank(name));
    // Shared words over the longer title's word count, ignoring little words
    // ("Day of the Viper" is not close to "Day of the Tentacle"): at least
    // half, and at least two words unless the title is a single word.
    const LITTLE: [&str; 8] = ["of", "the", "a", "an", "and", "in", "to", "on"];
    let significant = |words: &str| -> Vec<String> {
        words
            .split(' ')
            .filter(|word| !word.is_empty() && !LITTLE.contains(word))
            .map(str::to_owned)
            .collect()
    };
    let wanted_significant = significant(&wanted);
    let needed = wanted_significant.len().min(2);
    let mut close: Vec<(f64, &String)> = names
        .iter()
        .filter(|name| !exact.contains(name) && !main_title.contains(name))
        .filter_map(|name| {
            let words = significant(&game_title_words(name));
            let shared = wanted_significant.iter().filter(|word| words.contains(word)).count();
            let score = shared as f64 / wanted_significant.len().max(words.len()).max(1) as f64;
            (needed > 0 && shared >= needed && score >= 0.5).then_some((score, name))
        })
        .collect();
    close.sort_by(|(a, first), (b, second)| b.total_cmp(a).then_with(|| rank(first).cmp(&rank(second))));
    exact
        .into_iter()
        .chain(main_title)
        .chain(close.into_iter().map(|(_, name)| name))
        .map(String::as_str)
        .collect()
}

/// The best of `box_art_matches`.
pub fn best_box_art<'a>(game: &str, names: &'a [String]) -> Option<&'a str> {
    box_art_matches(game, names).into_iter().next()
}

fn box_art_url(platform: &str, name: &str) -> String {
    format!(
        "{LIBRETRO_THUMBNAILS}/{}/Named_Boxarts/{}.png",
        path_encode(platform),
        path_encode(name)
    )
}

/// Which tab of the chooser a candidate belongs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoverKind {
    /// A game's box art: for game music, whatever its format.
    Game,
    /// A music album's cover.
    Music,
}

impl CoverKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CoverKind::Game => "game",
            CoverKind::Music => "music",
        }
    }
}

/// A cover the listener can pick by hand: where it came from, what the
/// source calls it, and the image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverCandidate {
    pub kind: CoverKind,
    pub source: &'static str,
    pub title: String,
    pub url: String,
}

/// Every cover the sources offer for an album, for choosing by hand when the
/// automatic pick is wrong, both as a game and as music: the listener knows
/// which it is (a MIDI arrangement of a game's music is as much a game's as
/// an NSF), so both are offered whatever the format. Game: box art from the
/// format's consoles, and an image search for the game's box art. Music:
/// Deezer and Apple Music albums, and an image search for the album cover.
/// Only listings are fetched, not the images.
pub fn cover_candidates(
    file: &Path,
    artist: &str,
    album: &str,
    fetch: impl Fn(&str, u32) -> Option<Vec<u8>> + Sync,
) -> Vec<CoverCandidate> {
    let fetch = &fetch;
    let (game, music) = std::thread::scope(|scope| {
        let game = scope.spawn(move || game_candidates(file, album, fetch));
        let music = music_candidates(artist, album, fetch);
        (game.join().unwrap_or_default(), music)
    });
    let mut candidates = game;
    candidates.extend(music);
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|candidate| seen.insert((candidate.kind, candidate.url.clone())));
    candidates
}

fn image_search_candidates(
    kind: CoverKind,
    query: &str,
    fetch: &(impl Fn(&str, u32) -> Option<Vec<u8>> + Sync),
) -> Vec<CoverCandidate> {
    let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut candidates = Vec::new();
    if let Some(page) = fetch(&ddg_page_url(&query), MAX_SEARCH_BYTES)
        && let Ok(html) = String::from_utf8(page)
        && let Some(token) = ddg_token(&html)
        && let Some(results) = fetch(&ddg_image_url(&query, &token), MAX_SEARCH_BYTES)
        && let Ok(text) = String::from_utf8(results)
    {
        for (title, url) in parse_ddg_images(&text).into_iter().take(12) {
            candidates.push(CoverCandidate { kind, source: "Image search", title, url });
        }
    }
    candidates
}

fn game_candidates(
    file: &Path,
    album: &str,
    fetch: &(impl Fn(&str, u32) -> Option<Vec<u8>> + Sync),
) -> Vec<CoverCandidate> {
    let mut candidates = Vec::new();
    let platforms = game_platforms(file);
    for platform in &platforms {
        if let Some(names) = box_art_index(platform, fetch) {
            for name in box_art_matches(album, &names).into_iter().take(8) {
                candidates.push(CoverCandidate {
                    kind: CoverKind::Game,
                    source: "Box art",
                    title: name.to_owned(),
                    url: box_art_url(platform, name),
                });
            }
        }
    }
    // The listing can fail to load (it is large) or miss the game: try the
    // guessed names the automatic lookup uses, a batch at a time.
    if candidates.is_empty() && !platforms.is_empty() {
        let urls = game_box_art_urls(album, &platforms);
        let hits: Vec<String> = std::thread::scope(|scope| {
            let probes: Vec<_> = urls
                .iter()
                .take(BOX_ART_BATCH * 2)
                .map(|url| {
                    scope.spawn(move || {
                        fetch(url, MAX_COVER_BYTES)
                            .filter(|bytes| sniff_image_kind(bytes).is_some())
                            .map(|_| url.clone())
                    })
                })
                .collect();
            probes
                .into_iter()
                .filter_map(|probe| probe.join().ok().flatten())
                .collect()
        });
        for url in hits {
            let title = percent_encoding::percent_decode_str(
                url.rsplit('/').next().unwrap_or_default().trim_end_matches(".png"),
            )
            .decode_utf8_lossy()
            .into_owned();
            candidates.push(CoverCandidate { kind: CoverKind::Game, source: "Box art", title, url });
        }
    }
    let title = without_tag_groups(album_lookup_name(album));
    let query = match platforms.first() {
        Some(platform) => format!("{title} {} box art", platform.rsplit(" - ").next().unwrap_or(platform)),
        None => format!("{title} video game box art"),
    };
    candidates.extend(image_search_candidates(CoverKind::Game, &query, fetch));
    candidates
}

fn music_candidates(
    artist: &str,
    album: &str,
    fetch: &(impl Fn(&str, u32) -> Option<Vec<u8>> + Sync),
) -> Vec<CoverCandidate> {
    let mut candidates = Vec::new();
    let lookup = album_lookup_name(album);
    let json = |url: String| -> serde_json::Value {
        fetch(&url, MAX_SEARCH_BYTES)
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    };
    let deezer = json(deezer_search_url(artist, lookup));
    for entry in deezer.get("data").and_then(serde_json::Value::as_array).into_iter().flatten() {
        let url = json_string(entry, "cover_xl");
        if !url.is_empty() {
            let by = entry.get("artist").map(|artist| json_string(artist, "name")).unwrap_or_default();
            candidates.push(CoverCandidate {
                kind: CoverKind::Music,
                source: "Deezer",
                title: format!("{} – {by}", json_string(entry, "title")),
                url,
            });
        }
    }
    let itunes = json(itunes_search_url(artist, lookup));
    for entry in itunes.get("results").and_then(serde_json::Value::as_array).into_iter().flatten() {
        let url = json_string(entry, "artworkUrl100").replace("100x100bb", "600x600bb");
        if !url.is_empty() {
            candidates.push(CoverCandidate {
                kind: CoverKind::Music,
                source: "Apple Music",
                title: format!("{} – {}", json_string(entry, "collectionName"), json_string(entry, "artistName")),
                url,
            });
        }
    }
    candidates.extend(image_search_candidates(CoverKind::Music, &format!("{artist} {lookup} album cover"), fetch));
    candidates
}

/// What the image-search fallback asks for: a game's box art on its
/// console, or an album's cover.
fn image_search_query(file: &Path, artist: &str, album: &str) -> String {
    let platform = game_platforms(file)
        .first()
        .map(|platform| platform.rsplit(" - ").next().unwrap_or(platform).to_owned());
    match &platform {
        Some(platform) => format!("{} {platform} box art", search_title(file, album)),
        None => format!("{artist} {album} album cover"),
    }
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

/// Undo a rejection: these bytes may be stored and shown again.
pub fn unreject_cover(cache_dir: &Path, bytes: &[u8]) {
    let hash = image_hash(bytes);
    let path = cache_dir.join(REJECTED_COVERS);
    if let Ok(list) = std::fs::read_to_string(&path) {
        let kept: Vec<&str> = list.lines().filter(|line| *line != hash).collect();
        let _ = std::fs::write(&path, kept.iter().map(|line| format!("{line}\n")).collect::<String>());
    }
}

/// Box art for a platform's games, listed once per run of the program.
fn box_art_index(
    platform: &str,
    fetch: &impl Fn(&str, u32) -> Option<Vec<u8>>,
) -> Option<std::sync::Arc<Vec<String>>> {
    static INDEXES: std::sync::Mutex<Vec<(String, std::sync::Arc<Vec<String>>)>> =
        std::sync::Mutex::new(Vec::new());
    if let Some((_, names)) = INDEXES.lock().ok()?.iter().find(|(name, _)| name == platform) {
        return Some(names.clone());
    }
    let html = String::from_utf8(fetch(&box_art_index_url(platform), MAX_BOX_ART_INDEX_BYTES)?).ok()?;
    let names = std::sync::Arc::new(parse_box_art_index(&html));
    if names.is_empty() {
        return None;
    }
    INDEXES.lock().ok()?.push((platform.to_owned(), names.clone()));
    Some(names)
}

fn path_encode(value: &str) -> String {
    const SEGMENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');
    percent_encoding::utf8_percent_encode(value, SEGMENT).to_string()
}

pub fn deezer_search_url(artist: &str, album: &str) -> String {
    format!(
        "https://api.deezer.com/search/album?q={}&limit=5",
        encode(&query_text(artist, album))
    )
}

pub fn itunes_search_url(artist: &str, album: &str) -> String {
    format!(
        "https://itunes.apple.com/search?term={}&media=music&entity=album&limit=5",
        encode(&query_text(artist, album))
    )
}

pub fn mb_release_group_url(artist: &str, album: &str) -> String {
    let mut terms = Vec::new();
    if !artist.trim().is_empty() {
        terms.push(format!("artist:\"{}\"", artist.trim()));
    }
    terms.push(format!("releasegroup:\"{}\"", album.trim()));
    format!(
        "https://musicbrainz.org/ws/2/release-group/?query={}&fmt=json&limit=5",
        encode(&terms.join(" AND "))
    )
}

pub fn mb_release_url(artist: &str, album: &str) -> String {
    let mut terms = Vec::new();
    if !artist.trim().is_empty() {
        terms.push(format!("artist:\"{}\"", artist.trim()));
    }
    terms.push(format!("release:\"{}\"", album.trim()));
    format!(
        "https://musicbrainz.org/ws/2/release/?query={}&fmt=json&limit=5",
        encode(&terms.join(" AND "))
    )
}

pub fn caa_front_url(mbid: &str) -> String {
    format!("https://coverartarchive.org/release-group/{mbid}/front-500")
}

pub fn caa_release_front_url(mbid: &str) -> String {
    format!("https://coverartarchive.org/release/{mbid}/front-500")
}

pub fn ddg_page_url(query: &str) -> String {
    format!(
        "https://duckduckgo.com/?q={}&iar=images&iax=images&ia=images",
        encode(query)
    )
}

pub fn ddg_image_url(query: &str, token: &str) -> String {
    format!(
        "https://duckduckgo.com/i.js?q={}&vqd={}&p=1",
        encode(query),
        encode(token)
    )
}

/// Extract the `vqd` token the image endpoint requires from a search page.
pub fn ddg_token(page: &str) -> Option<String> {
    for (marker, quote) in [("vqd=\"", '"'), ("vqd='", '\'')] {
        let Some(start) = page.find(marker) else {
            continue;
        };
        let rest = &page[start + marker.len()..];
        let end = rest.find(quote)?;
        let token = &rest[..end];
        if !token.is_empty() && token.len() <= 128 {
            return Some(token.to_owned());
        }
    }
    None
}

fn json_string(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// (title, artist, cover) of the first Deezer album hit, if any.
pub fn parse_deezer_cover(json: &str) -> Option<(String, String, String)> {
    let root: serde_json::Value = serde_json::from_str(json).ok()?;
    let first = root.get("data")?.as_array()?.first()?;
    Some((
        json_string(first, "title"),
        first
            .get("artist")
            .map(|artist| json_string(artist, "name"))
            .unwrap_or_default(),
        json_string(first, "cover_xl"),
    ))
}

/// (collection, artist, artwork) of the first iTunes album hit, with the
/// 100px artwork URL upgraded to 600px.
pub fn parse_itunes_cover(json: &str) -> Option<(String, String, String)> {
    let root: serde_json::Value = serde_json::from_str(json).ok()?;
    let first = root.get("results")?.as_array()?.first()?;
    let artwork = json_string(first, "artworkUrl100").replace("100x100bb", "600x600bb");
    Some((
        json_string(first, "collectionName"),
        json_string(first, "artistName"),
        artwork,
    ))
}

/// MusicBrainz candidates in search order. Search rank is useful, but the
/// first hit may be a different album or a release without a front cover.
fn parse_mb_candidates(json: &str, key: &str) -> Vec<(String, String, String)> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    root.get(key)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|entry| {
            let artist = entry
                .get("artist-credit")
                .and_then(serde_json::Value::as_array)
                .and_then(|credit| credit.first())
                .map(|credit| json_string(credit, "name"))
                .unwrap_or_default();
            (
                json_string(entry, "title"),
                artist,
                json_string(entry, "id"),
            )
        })
        .collect()
}

pub fn parse_mb_release_groups(json: &str) -> Vec<(String, String, String)> {
    parse_mb_candidates(json, "release-groups")
}

pub fn parse_mb_releases(json: &str) -> Vec<(String, String, String)> {
    parse_mb_candidates(json, "releases")
}

/// (title, proxied thumbnail) of the first DuckDuckGo image hit. Only the
/// same-host thumbnail is used so the fetch allowlist stays closed.
/// Image search results as (title, image URL) pairs, in DuckDuckGo's order.
/// Each image is fetched through DuckDuckGo's image proxy, so only
/// DuckDuckGo's own hosts are contacted, at the original's resolution.
pub fn parse_ddg_images(json: &str) -> Vec<(String, String)> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(results) = root.get("results").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|result| {
            let image = json_string(result, "image");
            (image.starts_with("https://") || image.starts_with("http://")).then(|| {
                (
                    json_string(result, "title"),
                    format!("https://external-content.duckduckgo.com/iu/?u={}&f=1", encode(&image)),
                )
            })
        })
        .collect()
}

fn normalize(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect()
}

/// Accept a provider hit only when its title really matches the queried
/// album (either direction, so "Original Soundtrack" suffixes pass) and,
/// when both sides name an artist, the artists match too. This rejects
/// fuzzy mismatches like "Alinia" for an "Alisia Dragoon" query.
pub fn titles_match(
    query_artist: &str,
    query_album: &str,
    candidate_title: &str,
    candidate_artist: &str,
) -> bool {
    let album = normalize(query_album);
    let title = normalize(candidate_title);
    if album.len() < 3 || title.is_empty() {
        return false;
    }
    if !title.contains(&album) && !album.contains(&title) {
        return false;
    }
    let query = normalize(query_artist);
    let candidate = normalize(candidate_artist);
    if query.len() < 2 || candidate.is_empty() {
        return true;
    }
    query.contains(&candidate) || candidate.contains(&query)
}

/// When a soundtrack tags each track with its composer instead of the album
/// artist, an exact album title is still safe to use for a release lookup.
pub fn album_title_exact(query_album: &str, candidate_title: &str) -> bool {
    let album = normalize(query_album);
    album.len() >= 8 && album == normalize(candidate_title)
}

/// Exact-title release variants are safe to try when they all name the same
/// album artist. MusicBrainz can list several editions of one soundtrack;
/// requiring a single exact hit discards their usable front covers.
pub fn consistent_exact_releases(releases: &[(String, String, String)], album: &str) -> bool {
    let exact: Vec<_> = releases
        .iter()
        .filter(|(title, _, _)| album_title_exact(album, title))
        .collect();
    if exact.len() == 1 {
        return true;
    }
    let Some(artist) = exact.first().map(|(_, artist, _)| artist.trim()) else {
        return false;
    };
    !artist.is_empty()
        && exact
            .iter()
            .all(|(_, candidate, _)| normalize(candidate) == normalize(artist))
}

/// First embedded picture of a tagged file, if it decodes as JPEG/PNG.
/// Module music and untagged files yield nothing; the download chain covers
/// those.
pub fn embedded_cover_bytes(file_path: &Path) -> Option<Vec<u8>> {
    use lofty::file::TaggedFileExt;
    let tagged = lofty::read_from_path(file_path).ok()?;
    let picture = tagged.tags().iter().find_map(|tag| {
        tag.pictures()
            .iter()
            .find(|picture| picture.pic_type() == lofty::picture::PictureType::CoverFront)
            .or_else(|| tag.pictures().first())
    })?;
    let data = picture.data().to_vec();
    if sniff_image_kind(&data).is_some() {
        Some(data)
    } else {
        None
    }
}

/// Query text for providers that need one: the album, or the parent folder
/// name when tags are missing (common for chiptune rips).
pub fn fallback_album(file_path: &Path, album: &str) -> String {
    if !album.trim().is_empty() {
        return album.trim().to_owned();
    }
    file_path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// Multi-disc rips often append a disc number to an otherwise searchable
/// release title. Keep the original tag for the cache key and display name.
pub fn album_lookup_name(album: &str) -> &str {
    let album = album.trim();
    for (opening, closing) in [(" (", ')'), (" [", ']')] {
        if let Some((title, suffix)) = album.rsplit_once(opening)
            && let Some(label) = suffix.strip_suffix(closing)
        {
            let label = label.trim().to_ascii_lowercase();
            if ["disc", "cd"].iter().any(|prefix| {
                label.strip_prefix(prefix).is_some_and(|number| {
                    number
                        .trim()
                        .starts_with(|character: char| character.is_ascii_digit())
                })
            }) {
                return title.trim_end();
            }
        }
    }
    album
}

#[cfg(any(test, feature = "test-util"))]
mod tests {
    use super::*;

    #[test]
    fn game_music_tries_the_games_box_art_first() {
        let urls = game_box_art_urls(
            "Metroid (1986-08)(Nintendo R&D1)(Nintendo)",
            &game_platforms(Path::new("/music/Metroid/01 Title.nsf")),
        );
        assert_eq!(
            urls[1],
            "https://thumbnails.libretro.com/Nintendo%20-%20Nintendo%20Entertainment%20System/Named_Boxarts/Metroid%20%28USA%29.png"
        );
        assert!(game_platforms(Path::new("/music/song.mp3")).is_empty());
        // Folder names never decide: a MIDI is no console's.
        assert!(game_platforms(Path::new("/Chiptunes/gamemids/Day of the Tentacle/04 - The Inn.MID")).is_empty());
        assert!(game_platforms(Path::new("/vgmusic/Nintendo Consoles/Nintendo/Crystalis/001.mid")).is_empty());
    }

    #[test]
    fn game_music_searches_box_art_before_music_albums() {
        let file = Path::new(
            "/NES/Crystalis [God Slayer - Haruka Tenkuu no Sonata] (1990-04-13)(SNK)[NES]/Crystalis.nsf",
        );
        let album = "Crystalis [God Slayer - Haruka Tenkuu no Sonata] (1990-04-13)(SNK)";
        assert_eq!(
            image_search_query(file, "Tsutomu Ishida", album),
            "Crystalis Nintendo Entertainment System box art"
        );
        let requested = std::sync::Mutex::new(Vec::new());
        let cover = download_cover(
            file,
            "Tsutomu Ishida",
            album,
            |url, _| {
                requested.lock().unwrap().push(url.to_owned());
                if url.starts_with("https://duckduckgo.com/?") {
                    Some(b"vqd=\"4-123\"".to_vec())
                } else if url.starts_with("https://duckduckgo.com/i.js") {
                    Some(br#"{"results":[{"title":"Crystalis NES box art","image":"https://media.example/crystalis.jpg"}]}"#.to_vec())
                } else if url.starts_with("https://external-content.duckduckgo.com/") {
                    Some(vec![0xff, 0xd8, 0xff, 0xe0])
                } else if url.starts_with("https://api.deezer.com/") {
                    Some(br#"{"data":[{"title":"Crystalis","artist":{"name":"Some Band"},"cover_xl":"https://example.test/band.jpg"}]}"#.to_vec())
                } else if url.ends_with("/band.jpg") {
                    Some(vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
                } else {
                    None
                }
            },
            || false,
            Some,
        );
        assert_eq!(cover, Some(vec![0xff, 0xd8, 0xff, 0xe0]));
        assert!(
            !requested
                .into_inner()
                .unwrap()
                .iter()
                .any(|url| url.starts_with("https://api.deezer.com/"))
        );
    }

    #[test]
    fn shared_download_rejects_bad_image_and_uses_next_provider() {
        let requested = std::sync::Mutex::new(Vec::new());
        let cover = download_cover(
            Path::new("/music/song.mp3"),
            "Artist",
            "Album",
            |url, limit| {
                requested.lock().unwrap().push((url.to_owned(), limit));
                if url.starts_with("https://api.deezer.com/") {
                    Some(br#"{"data":[{"title":"Album","artist":{"name":"Artist"},"cover_xl":"https://example.test/deezer.jpg"}]}"#.to_vec())
                } else if url.ends_with("/deezer.jpg") {
                    Some(b"not an image".to_vec())
                } else if url.starts_with("https://itunes.apple.com/") {
                    Some(br#"{"results":[{"collectionName":"Album","artistName":"Artist","artworkUrl100":"https://example.test/100x100bb.jpg"}]}"#.to_vec())
                } else if url.ends_with("/600x600bb.jpg") {
                    Some(vec![0xff, 0xd8, 0xff, 0xe0])
                } else {
                    None
                }
            },
            || false,
            Some,
        );
        assert_eq!(cover, Some(vec![0xff, 0xd8, 0xff, 0xe0]));
        let requested = requested.into_inner().unwrap();
        assert_eq!(requested.len(), 4);
        assert_eq!(requested[0].1, MAX_SEARCH_BYTES);
        assert_eq!(requested[1].1, MAX_COVER_BYTES);
        assert!(requested[2].0.starts_with("https://itunes.apple.com/"));
    }

    #[test]
    fn shared_download_can_use_album_only_release_fallback() {
        let requested = std::sync::Mutex::new(Vec::new());
        let cover = download_cover(
            Path::new("/music/song.mp3"),
            "Track Composer",
            "Soundtrack Album",
            |url, _| {
                requested.lock().unwrap().push(url.to_owned());
                if url.starts_with("https://musicbrainz.org/ws/2/release/?query=")
                    && !url.contains("artist%3A")
                {
                    Some(br#"{"releases":[{"id":"release-1","title":"Soundtrack Album","artist-credit":[{"name":"Album Orchestra"}]}]}"#.to_vec())
                } else if url.ends_with("/release-1/front-500") {
                    Some(vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
                } else {
                    None
                }
            },
            || false,
            Some,
        );
        assert_eq!(
            cover,
            Some(vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
        );
        assert!(
            requested
                .into_inner()
                .unwrap()
                .iter()
                .any(|url| url.ends_with("/release-1/front-500"))
        );
    }

    #[test]
    fn sniff_rejects_non_images() {
        assert_eq!(
            sniff_image_kind(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 1]),
            Some(ImageKind::Jpeg)
        );
        assert_eq!(
            sniff_image_kind(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            Some(ImageKind::Png)
        );
        assert_eq!(sniff_image_kind(b"<html>nope</html>"), None);
        assert_eq!(sniff_image_kind(&[]), None);
        assert_eq!(sniff_image_kind(&[0xFF, 0xD8]), None);
    }

    #[test]
    fn cache_key_is_stable_and_filesafe() {
        let left = cache_key("Fumihito Kasatani", "Alisia Dragoon");
        assert_eq!(left, cache_key("Fumihito Kasatani", "Alisia Dragoon"));
        assert!(left.len() <= 60 + 1 + 16);
        assert!(!left.contains('/') && !left.contains('\0'));
        assert_ne!(left, cache_key("Fumihito Kasatani", "Alisia Dragoon!"));
        assert_ne!(left, cache_key("", "Alisia Dragoon"));
    }

    #[test]
    fn box_art_listing_matches_titles_loosely() {
        let html = r#"<li><a href="/Nintendo/"> Parent</a></li>
<li><a href="Vs.%20Super%20Mario%20Bros.%20(1986)(Nintendo)(VS%20UniSystem)%5bb2%5d.png">x</a></li>
<li><a href="Vs.%20Super%20Mario%20Bros.%20(1986)(Nintendo)(VS%20UniSystem).png">x</a></li>
<li><a href="Super%20Mario%20Bros.%203%20(USA).png">x</a></li>"#;
        let names = parse_box_art_index(html);
        assert_eq!(names.len(), 3);
        assert_eq!(
            best_box_art("VS. Super Mario Bros", &names),
            Some("Vs. Super Mario Bros. (1986)(Nintendo)(VS UniSystem)")
        );
        // No exact title: the closest one, so the cover shows something.
        assert_eq!(
            best_box_art("Super Mario Bros. 1/2/Vs", &names),
            Some("Vs. Super Mario Bros. (1986)(Nintendo)(VS UniSystem)")
        );
        assert_eq!(best_box_art("Mega Man", &names), None);
        assert_eq!(game_title_words("The Legend of Zelda"), "legend of zelda");
        let dos = vec![
            "Ultima IV - Quest of the Avatar".to_owned(),
            "Ultima V - Warriors of Destiny".to_owned(),
            "Ultima VII_ Part 1_ The Black Gate + Forge of Virtue".to_owned(),
            "Ultima Underworld - The Stygian Abyss".to_owned(),
        ];
        assert_eq!(best_box_art("Ultima 5", &dos), Some("Ultima V - Warriors of Destiny"));
        assert_eq!(
            best_box_art("Ultima 7", &dos),
            Some("Ultima VII_ Part 1_ The Black Gate + Forge of Virtue")
        );
        assert_eq!(game_title_words("Zelda, The"), "zelda");
    }

    #[test]
    fn rejected_covers_are_never_stored_again() {
        let dir = std::env::temp_dir().join(format!("kog-cover-reject-{}", std::process::id()));
        let jpeg = [0xff, 0xd8, 0xff, 0xe0, 1];
        let stored = store_cache(&dir, "album", &jpeg).unwrap();
        assert!(reject_cached_cover(&dir, &stored));
        assert!(cache_lookup(&dir, "album").is_none());
        assert!(store_cache(&dir, "album", &jpeg).is_none());
        assert!(store_cache(&dir, "album", &[0xff, 0xd8, 0xff, 0xe0, 2]).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cover_changes_are_followed_from_a_stamp() {
        let temporary = tempfile::tempdir().expect("temporary cache");
        let dir = temporary.path();
        let start = cover_changes_stamp(dir);
        assert_eq!(cover_changes_since(dir, start), (0, Vec::new()));
        let jpeg = [0xff, 0xd8, 0xff, 0xe0, 1];
        store_cache(dir, "album", &jpeg).unwrap();
        assert!(reject_cover(dir, &jpeg));
        note_cover_change(dir, None);
        let stamp = cover_changes_stamp(dir);
        assert_ne!(stamp, start);
        let (next, changes) = cover_changes_since(dir, start);
        assert_eq!(next, stamp);
        assert_eq!(changes, vec![Some(fnv1a64(&jpeg)), None]);
        assert_eq!(cover_changes_since(dir, next), (next, Vec::new()));
        // The lists are not covers: rejecting and evicting leave them be.
        evict_cache(dir, 0);
        assert_eq!(cover_changes_stamp(dir), stamp);
    }

    #[test]
    fn misses_are_remembered() {
        let dir = std::env::temp_dir().join(format!("kog-cover-miss-{}", std::process::id()));
        assert!(!recent_miss(&dir, "album"));
        record_miss(&dir, "album");
        assert!(recent_miss(&dir, "album"));
        assert!(!recent_miss(&dir, "other"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_roundtrip_and_eviction() {
        let temporary = tempfile::tempdir().expect("temporary cache");
        let dir = cache_directory(temporary.path());
        let key = cache_key("Artist", "Album");
        assert_eq!(cache_lookup(&dir, &key), None);
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0x01, 0x02];
        let stored = store_cache(&dir, &key, &jpeg).expect("store jpeg");
        assert_eq!(stored.extension().and_then(|ext| ext.to_str()), Some("jpg"));
        assert_eq!(cache_lookup(&dir, &key), Some(stored));
        assert_eq!(store_cache(&dir, &key, b"nope"), None);
        evict_cache(&dir, 1);
        assert_eq!(cache_lookup(&dir, &key), None);
        evict_cache(&dir, MAX_CACHE_BYTES);
    }

    #[test]
    fn provider_urls_encode_queries() {
        let deezer = deezer_search_url("Fumihito Kasatani", "Alisia Dragoon");
        assert!(deezer.starts_with("https://api.deezer.com/search/album?q="));
        assert!(deezer.contains("Alisia%20Dragoon"));
        let itunes = itunes_search_url("", "Alisia Dragoon");
        assert!(itunes.contains("entity=album") && itunes.contains("Alisia%20Dragoon"));
        let mb = mb_release_group_url("Kasatani", "Alisia Dragoon");
        assert!(mb.contains("musicbrainz.org/ws/2/release-group/"));
        assert!(mb.contains("fmt=json"));
        let release = mb_release_url("", "Super Mario Galaxy Original Soundtrack");
        assert!(release.contains("musicbrainz.org/ws/2/release/"));
        assert!(release.contains("release%3A%22Super%20Mario%20Galaxy"));
        assert_eq!(
            caa_front_url("abc-123"),
            "https://coverartarchive.org/release-group/abc-123/front-500"
        );
        assert_eq!(
            caa_release_front_url("abc-123"),
            "https://coverartarchive.org/release/abc-123/front-500"
        );
        let ddg = ddg_image_url("Alisia Dragoon box art", "tok");
        assert!(ddg.starts_with("https://duckduckgo.com/i.js?"));
        assert!(ddg.contains("vqd=tok"));
    }

    #[test]
    fn deezer_parse_reads_first_hit() {
        let parsed = parse_deezer_cover(
            r#"{"data":[{"title":"Alisia Dragoon","artist":{"name":"Fumihito Kasatani"},"cover_xl":"https://cdn-images.dzcdn.net/xl.jpg"}]}"#,
        )
        .expect("parse deezer");
        assert_eq!(parsed.0, "Alisia Dragoon");
        assert_eq!(parsed.1, "Fumihito Kasatani");
        assert_eq!(parsed.2, "https://cdn-images.dzcdn.net/xl.jpg");
        assert_eq!(parse_deezer_cover(r#"{"data":[]}"#), None);
    }

    #[test]
    fn itunes_parse_upgrades_artwork() {
        let parsed = parse_itunes_cover(
            r#"{"results":[{"collectionName":"Alisia Dragoon","artistName":"Foo","artworkUrl100":"https://is1.mzstatic.com/a/100x100bb.jpg"}]}"#,
        )
        .expect("parse itunes");
        assert_eq!(parsed.2, "https://is1.mzstatic.com/a/600x600bb.jpg");
    }

    #[test]
    fn mb_parse_reads_release_candidates() {
        let parsed = parse_mb_release_groups(
            r#"{"release-groups":[{"id":"abc-123","title":"Alisia Dragoon","artist-credit":[{"name":"Kasatani"}]}]}"#,
        );
        assert_eq!(parsed, vec![("Alisia Dragoon".to_owned(), "Kasatani".to_owned(), "abc-123".to_owned())]);
        let releases = parse_mb_releases(
            r#"{"releases":[{"id":"wrong","title":"Other","artist-credit":[{"name":"Other"}]},{"id":"platinum","title":"Super Mario Galaxy Original Soundtrack: Platinum Version","artist-credit":[{"name":"Mario Galaxy Orchestra"}]}]}"#,
        );
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[1].2, "platinum");
        assert!(parse_mb_release_groups(r#"{"release-groups":[]}"#).is_empty());
    }

    #[test]
    fn ddg_token_and_thumbnail_parse() {
        let page = r#"<html><script>vqd="4-123abcDEF_-&more";</script></html>"#;
        assert_eq!(ddg_token(page), Some("4-123abcDEF_-&more".to_owned()));
        assert_eq!(ddg_token("<html>none</html>"), None);
        let parsed = parse_ddg_images(
            r#"{"results":[{"title":"Ultima 5 box","image":"https://media.example/u5.jpg"},{"title":"x","image":"javascript:1"}]}"#,
        );
        assert_eq!(
            parsed,
            vec![(
                "Ultima 5 box".to_owned(),
                "https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Fmedia.example%2Fu5.jpg&f=1".to_owned()
            )]
        );
    }

    #[test]
    fn title_match_rejects_fuzzy_misses() {
        // Live Deezer miss: "Alinia" must not pass for "Alisia Dragoon".
        assert!(!titles_match("", "Alisia Dragoon", "Alinia", "Some Artist"));
        assert!(titles_match("", "Alisia Dragoon", "Alisia Dragoon", ""));
        assert!(titles_match(
            "",
            "Alisia Dragoon",
            "Alisia Dragoon (Original Game Soundtrack)",
            ""
        ));
        assert!(titles_match("Fumihito Kasatani", "Alisia Dragoon", "Alisia Dragoon", "Kasatani"));
        assert!(!titles_match("Kasatani", "Alisia Dragoon", "Alisia Dragoon", "Unrelated Person"));
        assert!(!titles_match("", "", "Anything", ""));
        assert!(!titles_match("", "AB", "AB", ""));
        assert!(!titles_match("", "Alisia Dragoon", "", ""));
        assert!(album_title_exact(
            "Super Mario Galaxy Original Soundtrack Platinum Version",
            "Super Mario Galaxy Original Soundtrack: Platinum Version"
        ));
        assert!(!album_title_exact("Super Mario Galaxy", "Super Mario Galaxy 2"));
    }

    #[test]
    fn exact_release_editions_can_share_an_album_artist() {
        let album = "Super Mario Galaxy";
        let mut releases = vec![
            (album.to_owned(), "Mario Galaxy Orchestra".to_owned(), "one".to_owned()),
            (album.to_owned(), "Mario Galaxy Orchestra".to_owned(), "two".to_owned()),
            ("Super Mario Galaxy 2".to_owned(), "Other".to_owned(), "other".to_owned()),
        ];
        assert!(consistent_exact_releases(&releases, album));
        releases[1].1 = "Different Artist".to_owned();
        assert!(!consistent_exact_releases(&releases, album));
    }

    #[test]
    fn fallback_album_prefers_tags_then_folder() {
        assert_eq!(
            fallback_album(Path::new("/music/rip/song.flac"), "Real Album"),
            "Real Album"
        );
        assert_eq!(
            fallback_album(Path::new("/music/Alisia Dragoon/song.flac"), "  "),
            "Alisia Dragoon"
        );
        assert_eq!(fallback_album(Path::new("/song.flac"), ""), "");
    }

    #[test]
    fn lookup_name_omits_only_a_disc_suffix() {
        assert_eq!(
            album_lookup_name("Super Mario Galaxy Original Soundtrack Platinum Version (Disc 2)"),
            "Super Mario Galaxy Original Soundtrack Platinum Version"
        );
        assert_eq!(album_lookup_name("Album [CD 1]"), "Album");
        assert_eq!(album_lookup_name("Album (Deluxe Edition)"), "Album (Deluxe Edition)");
    }

    fn write_jpeg(path: &Path) {
        std::fs::write(path, [0xFF, 0xD8, 0xFF, 0xE0, 0x01, 0x02]).expect("write jpeg");
    }

    fn write_png(path: &Path) {
        std::fs::write(path, [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]).expect("write png");
    }

    #[test]
    fn sibling_art_prefers_named_files_beside_the_track() {
        let temporary = tempfile::tempdir().expect("temporary music");
        let track = temporary.path().join("01.flac");
        std::fs::write(&track, []).expect("write track");
        // Case-insensitive exact stems win over loose images.
        write_jpeg(&temporary.path().join("Folder.JPG"));
        write_png(&temporary.path().join("random.png"));
        let found = sibling_cover_bytes(&track).expect("sibling art");
        assert_eq!(sniff_image_kind(&found), Some(ImageKind::Jpeg));
    }

    #[test]
    fn sibling_art_skips_junk_names_and_back_scans() {
        let temporary = tempfile::tempdir().expect("temporary music");
        let track = temporary.path().join("01.flac");
        std::fs::write(&track, []).expect("write track");
        // Junk front art falls through to the valid cover.
        std::fs::write(temporary.path().join("front.jpg"), b"not an image").expect("write junk");
        write_png(&temporary.path().join("cover.png"));
        let found = sibling_cover_bytes(&track).expect("sibling art");
        assert_eq!(sniff_image_kind(&found), Some(ImageKind::Png));
        // A covers/ folder with only booklet/back/disc scans resolves to
        // the first non-rejected image, never the back cover.
        std::fs::remove_file(temporary.path().join("front.jpg")).unwrap();
        std::fs::remove_file(temporary.path().join("cover.png")).unwrap();
        let scans = temporary.path().join("covers");
        std::fs::create_dir(&scans).expect("covers dir");
        for name in [
            "b1.png",
            "b2.png",
            "back.png",
            "c1.png",
            "c2.png",
            "cdscan.png",
        ] {
            write_png(&scans.join(name));
        }
        std::fs::remove_file(scans.join("b1.png")).unwrap();
        std::fs::remove_file(scans.join("b2.png")).unwrap();
        std::fs::remove_file(scans.join("c1.png")).unwrap();
        std::fs::remove_file(scans.join("c2.png")).unwrap();
        // Only back/disc scans left: nothing promotable.
        assert_eq!(sibling_cover_bytes(&track), None);
    }

    #[test]
    fn sibling_art_falls_back_to_first_loose_scan() {
        let temporary = tempfile::tempdir().expect("temporary music");
        let track = temporary.path().join("01.flac");
        std::fs::write(&track, []).expect("write track");
        let scans = temporary.path().join("covers");
        std::fs::create_dir(&scans).expect("covers dir");
        for name in [
            "b1.png",
            "b2.png",
            "back.png",
            "c1.png",
            "c2.png",
            "cdscan.png",
        ] {
            write_png(&scans.join(name));
        }
        assert!(sibling_cover_bytes(&track).is_some());
    }

    #[test]
    fn sibling_art_ignores_missing_files_and_oversized_images() {
        assert_eq!(
            sibling_cover_bytes(Path::new("/music/nowhere/song.flac")),
            None
        );
        let temporary = tempfile::tempdir().expect("temporary music");
        let track = temporary.path().join("01.flac");
        std::fs::write(&track, []).expect("write track");
        let big = vec![0xFF_u8; MAX_COVER_BYTES as usize + 1];
        std::fs::write(temporary.path().join("folder.jpg"), &big).expect("write big");
        assert_eq!(sibling_cover_bytes(&track), None);
    }

    #[test]
    fn folder_cover_key_scopes_to_folders() {
        let left = folder_cover_key(Path::new("/music/rip"));
        assert_eq!(left, folder_cover_key(Path::new("/music/rip")));
        assert_ne!(left, folder_cover_key(Path::new("/music/other")));
        assert_ne!(left, cache_key("Artist", "Album"));
    }
}
