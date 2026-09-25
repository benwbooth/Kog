//! Resolve the same local cover sources as Qt and reduce them to an ANSI
//! half-block preview. This runs on the cover worker, never while drawing.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use directories::ProjectDirs;
use image::imageops::FilterType;
use kog_audio::cover_art;

pub const COVER_WIDTH: usize = 64;
pub const COVER_HEIGHT: usize = 64;

#[derive(Clone)]
pub struct CoverPreview {
    pub pixels: [[u8; 3]; COVER_WIDTH * COVER_HEIGHT],
    pub path: PathBuf,
}

fn preview(bytes: &[u8], path: PathBuf) -> Option<CoverPreview> {
    let image = image::load_from_memory(bytes).ok()?;
    let small = image
        .resize_exact(
            COVER_WIDTH as u32,
            COVER_HEIGHT as u32,
            FilterType::Triangle,
        )
        .to_rgb8();
    let mut pixels = [[0; 3]; COVER_WIDTH * COVER_HEIGHT];
    for (index, pixel) in small.pixels().enumerate() {
        pixels[index] = pixel.0;
    }
    Some(CoverPreview { pixels, path })
}

fn cached(cache_dir: &Path, key: &str) -> Option<CoverPreview> {
    let path = cover_art::cache_lookup(cache_dir, key)?;
    let bytes = std::fs::read(&path).ok()?;
    preview(&bytes, path)
}

fn stored(cache_dir: &Path, key: &str, bytes: &[u8]) -> Option<CoverPreview> {
    let path = cover_art::store_cache(cache_dir, key, bytes)?;
    preview(bytes, path)
}

fn fetch(url: &str, max_bytes: u32) -> Option<Vec<u8>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .build()
        .into();
    let mut response = None;
    for attempt in 0..2 {
        match agent
            .get(url)
            .header(
                "User-Agent",
                concat!("Kog/", env!("CARGO_PKG_VERSION"), " (terminal cover art)"),
            )
            .call()
        {
            Ok(found) => {
                response = Some(found);
                break;
            }
            Err(ureq::Error::StatusCode(429 | 500 | 502 | 503 | 504)) if attempt == 0 => {
                std::thread::sleep(Duration::from_millis(1200));
            }
            Err(_) => return None,
        }
    }
    let response = response?;
    let mut bytes = Vec::new();
    response
        .into_body()
        .as_reader()
        .take(u64::from(max_bytes) + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= max_bytes as usize).then_some(bytes)
}

fn download(
    cache_dir: &Path,
    key: &str,
    artist: &str,
    album: &str,
    cancelled: &impl Fn() -> bool,
) -> Option<CoverPreview> {
    cover_art::download_cover(artist, album, fetch, cancelled, |bytes| {
        stored(cache_dir, key, &bytes)
    })
}

pub fn resolve(
    file: Option<&Path>,
    artist: &str,
    tagged_album: &str,
    allow_download: bool,
    cancelled: impl Fn() -> bool,
) -> Option<CoverPreview> {
    let project = ProjectDirs::from("org", "Kog", "Kog")?;
    let cache_dir = cover_art::cache_directory(project.cache_dir());
    let album = file.map_or_else(
        || tagged_album.to_owned(),
        |file| cover_art::fallback_album(file, tagged_album),
    );
    if album.is_empty() {
        return None;
    }
    let (key, may_download) = file.map_or_else(
        || (cover_art::cache_key(artist, &album), true),
        |file| cover_art::track_cache_key(artist, tagged_album, &album, file),
    );
    if let Some(preview) = cached(&cache_dir, &key) {
        return Some(preview);
    }
    if let Some(file) = file.filter(|path| path.is_file()) {
        if let Some(bytes) = cover_art::embedded_cover_bytes(file)
            && let Some(preview) = stored(&cache_dir, &key, &bytes)
        {
            return Some(preview);
        }
        if let Some(folder) = file.parent() {
            let folder_key = cover_art::folder_cover_key(folder);
            if let Some(preview) = cached(&cache_dir, &folder_key) {
                return Some(preview);
            }
            if let Some(bytes) = cover_art::sibling_cover_bytes(file)
                && let Some(preview) = stored(&cache_dir, &folder_key, &bytes)
            {
                return Some(preview);
            }
        }
    }
    if allow_download && may_download {
        download(&cache_dir, &key, artist, &album, &cancelled)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn decodes_color_into_half_block_pixels() {
        let image = image::RgbImage::from_pixel(4, 4, image::Rgb([240, 25, 60]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let result = preview(&bytes, PathBuf::from("cover.png")).unwrap();
        assert_eq!(result.pixels, [[240, 25, 60]; COVER_WIDTH * COVER_HEIGHT]);
    }

    #[test]
    fn retries_a_transient_cover_provider_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for status in ["503 Service Unavailable", "200 OK"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 512];
                let _ = stream.read(&mut request).unwrap();
                let body = if status == "200 OK" { "cover" } else { "" };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        assert_eq!(
            fetch(&format!("http://{address}/cover"), 16),
            Some(b"cover".to_vec())
        );
        server.join().unwrap();
    }

    #[test]
    #[ignore = "requires external HTTPS access"]
    fn live_https_fetch_smoke() {
        assert!(fetch("https://www.example.com/", cover_art::MAX_SEARCH_BYTES).is_some());
    }

    #[test]
    #[ignore = "requires external cover providers"]
    fn live_super_mario_galaxy_cover() {
        let cache = tempfile::tempdir().unwrap();
        let cover = download(
            cache.path(),
            "super-mario-galaxy",
            "Koji Kondo",
            "Super Mario Galaxy",
            &|| false,
        );
        assert!(
            cover.is_some(),
            "no provider returned a matching Super Mario Galaxy cover"
        );
    }
}
