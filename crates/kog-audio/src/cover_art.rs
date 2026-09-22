//! Album cover resolution: embedded tags, then a disk cache, then download
//! providers (Deezer, iTunes, MusicBrainz/Cover Art Archive, DuckDuckGo).
//! All network access goes through the worker in app_controller; everything
//! here is synchronous and side-effect free except the cache helpers.

use std::path::{Path, PathBuf};

pub const MAX_COVER_BYTES: u32 = 8 * 1024 * 1024;
pub const MAX_SEARCH_BYTES: u32 = 512 * 1024;
pub const MAX_CACHE_BYTES: u64 = 200 * 1024 * 1024;
const CACHE_SUBDIRECTORY: &str = "covers";

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
    } else if bytes.len() >= 8
        && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
    {
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
    let mut stem = sanitize_component(&format!(
        "{} - {}",
        artist.trim(),
        album.trim()
    ));
    if stem.is_empty() {
        stem.push_str("untitled");
    }
    let truncated: String = stem.chars().take(60).collect();
    format!("{truncated}-{hash:016x}")
}

pub fn cache_directory(base: &Path) -> PathBuf {
    base.join(CACHE_SUBDIRECTORY)
}

/// Directory names that commonly hold scanned artwork next to music.
const ART_SUBDIRECTORIES: [&str; 5] = ["covers", "cover", "artwork", "scans", "scan"];
/// File stems preferred as the primary artwork, most specific first.
const PREFERRED_ART_STEMS: [&str; 6] =
    ["front", "frontcover", "cover", "folder", "album", "coverart"];
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

/// Validate and persist downloaded or embedded bytes. Runs cache eviction
/// first so the budget holds even on a full disk of covers.
pub fn store_cache(cache_dir: &Path, key: &str, bytes: &[u8]) -> Option<PathBuf> {
    let kind = sniff_image_kind(bytes)?;
    if std::fs::create_dir_all(cache_dir).is_err() {
        return None;
    }
    evict_cache(cache_dir, MAX_CACHE_BYTES);
    let path = cache_dir.join(format!("{key}.{}", kind.extension()));
    if std::fs::write(&path, bytes).is_err() {
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
            if !metadata.is_file() {
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
    format!("{} {}", artist.trim(), album.trim()).trim().to_owned()
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
            (json_string(entry, "title"), artist, json_string(entry, "id"))
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
pub fn parse_ddg_thumbnail(json: &str) -> Option<(String, String)> {
    let root: serde_json::Value = serde_json::from_str(json).ok()?;
    let first = root.get("results")?.as_array()?.first()?;
    let thumbnail = json_string(first, "thumbnail");
    if thumbnail.is_empty() || !thumbnail.starts_with("https://duckduckgo.com/") {
        return None;
    }
    Some((json_string(first, "title"), thumbnail))
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
                    number.trim().starts_with(|character: char| character.is_ascii_digit())
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
        let parsed = parse_ddg_thumbnail(
            r#"{"results":[{"title":"Alisia Dragoon boxart","thumbnail":"https://duckduckgo.com/iu/?u=https%3A%2F%2Fx.jpg&f=1"}]}"#,
        )
        .expect("parse ddg");
        assert!(parsed.1.starts_with("https://duckduckgo.com/"));
        assert_eq!(
            parse_ddg_thumbnail(r#"{"results":[{"title":"x","thumbnail":"https://evil.example/x.jpg"}]}"#),
            None
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
