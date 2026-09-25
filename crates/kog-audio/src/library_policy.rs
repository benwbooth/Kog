//! Rules for files discovered while adding a folder to a playlist.
//!
//! An explicitly opened playlist or cue sheet is always read; the user's
//! folder preferences apply only to files found during a recursive walk.

use std::path::Path;

pub fn include_discovered_file(
    path: &Path,
    read_cue_sheets: bool,
    read_playlists: bool,
    explicit_root: bool,
) -> bool {
    if explicit_root {
        return true;
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("cue") {
        return read_cue_sheets;
    }
    if matches!(extension.to_ascii_lowercase().as_str(), "m3u" | "m3u8" | "pls") {
        return read_playlists;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_files_bypass_folder_preferences() {
        assert!(!include_discovered_file(Path::new("album.CUE"), false, true, false));
        assert!(!include_discovered_file(Path::new("album.M3U"), true, false, false));
        assert!(include_discovered_file(Path::new("album.CUE"), false, false, true));
        assert!(include_discovered_file(Path::new("song.flac"), false, false, false));
    }
}
