//! Small covers for the desktop app's song rows and playlist list. The app
//! shows them at its icon size (18 points), so each is kept on disk as a
//! little PNG with its corners already rounded: Qt loads it as an ordinary
//! image, off the UI thread, with no effect to draw per row.
//!
//! A song's cover comes from what is already on disk, never from reading
//! the song through a decoder (chiptunes are played through to learn their
//! tags) or from the network: the album's cover in the cover cache (the
//! server's folder-and-album-tag key), the desktop app's own entry for the
//! song, the folder's, then a picture in the song's tags or beside it. The
//! same small image serves every song of the album. A playlist's icon is
//! made from the covers the server's playlist-cover worker keeps in
//! `playlist-tiles`, laid out as the phone's tiles are.
//!
//! Everything here reads files and decodes images, so the app calls it from
//! worker threads only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use image::RgbaImage;
use kog_audio::cover_art;

/// The icons' side in pixels: sharp up to four device pixels per point.
pub const ICON_SIZE: u32 = 72;
/// The corner radius, in pixels of [`ICON_SIZE`]: 3 points of 18.
const RADIUS: f32 = 12.0;
const THUMB_DIRECTORY: &str = "cover-thumbs";
const TILE_DIRECTORY: &str = "playlist-tiles";
const TILES_FILE: &str = "tiles.json";

fn fnv(hash: u64, bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(hash, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}

const FNV_START: u64 = 0xcbf2_9ce4_8422_2325;

fn beside(cover_cache: &Path, name: &str) -> PathBuf {
    cover_cache
        .parent()
        .map(|parent| parent.join(name))
        .unwrap_or_else(|| std::env::temp_dir().join(format!("kog-{name}")))
}

/// The cache key every song of an album shares: its folder and album tag
/// (the folder alone for a song without one), as the server keys them.
pub fn album_key(file: &Path, album: &str) -> String {
    crate::covers::album_cover_key(file, album)
}

fn readable(cover_cache: &Path, path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path)
        .ok()
        .filter(|bytes| cover_art::sniff_image_kind(bytes).is_some())
        .filter(|bytes| !cover_art::is_rejected(cover_cache, bytes))
}

/// The song's cover from what is on disk, or `None`.
fn song_cover(cover_cache: &Path, file: &Path, artist: &str, album: &str) -> Option<Vec<u8>> {
    let album_key = album_key(file, album);
    if let Some(bytes) = cover_art::cache_lookup(cover_cache, &album_key)
        .and_then(|path| readable(cover_cache, &path))
    {
        return Some(bytes);
    }
    let lookup_album = cover_art::fallback_album(file, album);
    let mut keys = Vec::new();
    if !lookup_album.is_empty() {
        keys.push(cover_art::track_cache_key(artist, album, &lookup_album, file).0);
    }
    if let Some(folder) = file.parent() {
        keys.push(cover_art::folder_cover_key(folder));
    }
    let cached = keys.iter().find_map(|key| {
        cover_art::cache_lookup(cover_cache, key).and_then(|path| readable(cover_cache, &path))
    });
    let bytes = cached.or_else(|| {
        cover_art::embedded_cover_bytes(file)
            .or_else(|| cover_art::sibling_cover_bytes(file))
            .filter(|bytes| !cover_art::is_rejected(cover_cache, bytes))
    })?;
    // The album's other songs (here and on the server) find it at once.
    let _ = cover_art::store_cache(cover_cache, &album_key, &bytes);
    Some(bytes)
}

/// Coverage of the pixel at (x, y) by a square of `size` with rounded
/// corners: 1 inside, 0 outside, anti-aliased along the curve.
fn coverage(x: u32, y: u32, size: u32) -> f32 {
    let centre = |value: u32| {
        let value = value as f32 + 0.5;
        let far = size as f32 - RADIUS;
        if value < RADIUS {
            RADIUS - value
        } else if value > far {
            value - far
        } else {
            0.0
        }
    };
    let (dx, dy) = (centre(x), centre(y));
    if dx == 0.0 || dy == 0.0 {
        return 1.0;
    }
    (RADIUS - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0)
}

fn round_corners(image: &mut RgbaImage) {
    let size = image.width();
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let alpha = coverage(x, y, size);
        if alpha < 1.0 {
            pixel.0[3] = (f32::from(pixel.0[3]) * alpha).round() as u8;
        }
    }
}

fn square(bytes: &[u8], side: u32) -> Option<RgbaImage> {
    let image = image::load_from_memory(bytes).ok()?;
    Some(
        image
            .resize_to_fill(side, side, image::imageops::FilterType::Triangle)
            .to_rgba8(),
    )
}

/// Write the icon `name`, unless it is already there.
fn keep(dir: &Path, name: &str, make: impl FnOnce() -> Option<RgbaImage>) -> Option<PathBuf> {
    let path = dir.join(name);
    if path.is_file() {
        return Some(path);
    }
    let mut icon = make()?;
    round_corners(&mut icon);
    std::fs::create_dir_all(dir).ok()?;
    let temporary = path.with_extension("tmp");
    icon.save_with_format(&temporary, image::ImageFormat::Png).ok()?;
    std::fs::rename(&temporary, &path).ok()?;
    Some(path)
}

/// The song's cover as a small rounded icon, or `None` for a song without
/// one on disk.
pub fn song_icon(cover_cache: &Path, file: &Path, artist: &str, album: &str) -> Option<PathBuf> {
    if !file.is_file() {
        return None;
    }
    let cover = song_cover(cover_cache, file, artist, album)?;
    let name = format!("{:016x}-r{ICON_SIZE}.png", fnv(FNV_START, &cover));
    keep(&beside(cover_cache, THUMB_DIRECTORY), &name, || square(&cover, ICON_SIZE))
}

/// When the playlist-cover worker last saved its tiles.
pub fn playlist_tiles_stamp(cover_cache: &Path) -> Option<SystemTime> {
    std::fs::metadata(beside(cover_cache, TILE_DIRECTORY).join(TILES_FILE))
        .and_then(|metadata| metadata.modified())
        .ok()
}

#[derive(serde::Deserialize)]
struct Tile {
    #[serde(default)]
    prints: Vec<u64>,
}

#[derive(serde::Deserialize)]
struct Tiles {
    #[serde(default)]
    by_id: HashMap<i64, Tile>,
}

/// Every playlist's icon (Favorites is id 0): the first cover of its tile,
/// for the playlists whose tile has one. A 2×2 grid is too small to read
/// at the sidebar's size, so the icon is one cover whatever the phone's
/// setting.
pub fn playlist_icons(cover_cache: &Path) -> HashMap<i64, PathBuf> {
    let tiles_dir = beside(cover_cache, TILE_DIRECTORY);
    let thumbs_dir = beside(cover_cache, THUMB_DIRECTORY);
    let Some(tiles) = std::fs::read(tiles_dir.join(TILES_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Tiles>(&bytes).ok())
    else {
        return HashMap::new();
    };
    let mut icons = HashMap::new();
    for (id, tile) in tiles.by_id {
        let Some(&print) = tile.prints.first() else {
            continue;
        };
        let name = format!("cover-{print:016x}-r{ICON_SIZE}.png");
        let icon = keep(&thumbs_dir, &name, || {
            square(&std::fs::read(tiles_dir.join(format!("{print:016x}"))).ok()?, ICON_SIZE)
        });
        if let Some(icon) = icon {
            icons.insert(id, icon);
        }
    }
    icons
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(colour: [u8; 3]) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbImage::from_pixel(300, 200, image::Rgb(colour))
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn song_icons_are_rounded_and_shared_by_the_album() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("covers");
        let album = root.path().join("album");
        std::fs::create_dir_all(&album).unwrap();
        std::fs::write(album.join("folder.png"), png([200, 30, 30])).unwrap();
        let first = album.join("01.ogg");
        let second = album.join("02.ogg");
        std::fs::write(&first, b"not really").unwrap();
        std::fs::write(&second, b"not really").unwrap();
        let icon = song_icon(&cache, &first, "Artist", "Album").unwrap();
        assert_eq!(song_icon(&cache, &second, "Artist", "Album"), Some(icon.clone()));
        let image = image::open(&icon).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (ICON_SIZE, ICON_SIZE));
        assert_eq!(image.get_pixel(0, 0).0[3], 0);
        assert_eq!(image.get_pixel(ICON_SIZE / 2, ICON_SIZE / 2).0, [200, 30, 30, 255]);
    }

    #[test]
    fn playlist_icons_show_the_tiles_first_cover() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("covers");
        let tiles = root.path().join(TILE_DIRECTORY);
        std::fs::create_dir_all(&tiles).unwrap();
        std::fs::write(tiles.join(format!("{:016x}", 1u64)), png([255, 0, 0])).unwrap();
        std::fs::write(tiles.join(format!("{:016x}", 2u64)), png([0, 0, 255])).unwrap();
        std::fs::write(
            tiles.join(TILES_FILE),
            r#"{"order":[0,5],"by_id":{"0":{"prints":[1,2]},"5":{"prints":[]}}}"#,
        )
        .unwrap();
        let icons = playlist_icons(&cache);
        assert_eq!(icons.len(), 1);
        let image = image::open(&icons[&0]).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (ICON_SIZE, ICON_SIZE));
        for (x, y) in [(1, 1), (3, 1), (1, 3), (3, 3)] {
            let at = image.get_pixel(x * ICON_SIZE / 4, y * ICON_SIZE / 4).0;
            assert_eq!(at, [255, 0, 0, 255]);
        }
    }
}
