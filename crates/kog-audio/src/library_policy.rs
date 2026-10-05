//! Rules for files discovered while adding a folder to a playlist.
//!
//! An explicitly opened playlist or cue sheet is always read; the user's
//! folder preferences apply only to files found during a recursive walk.

use std::path::Path;

/// A snapshot of a file-tree search when an add operation starts. A folder
/// whose own name matches exposes its descendants; a structural ancestor
/// only contributes matching descendants. Words must match one name, never
/// unrelated components of a path.
#[derive(Clone, Debug, Default)]
pub struct TreeFilter {
    pub root: std::path::PathBuf,
    words: Vec<String>,
}

impl TreeFilter {
    pub fn new(query: &str, root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            root: root.into(),
            words: query
                .to_lowercase()
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
        }
    }

    pub fn query(&self) -> String {
        self.words.join(" ")
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn matches_name(&self, name: &str) -> bool {
        let folded = name.to_lowercase();
        self.words.iter().all(|word| folded.contains(word))
    }

    pub fn matches_path(&self, path: &Path) -> bool {
        self.is_empty()
            || path
                .strip_prefix(&self.root)
                .unwrap_or(path)
                .components()
                .any(|part| self.matches_name(&part.as_os_str().to_string_lossy()))
    }
}

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
    if matches!(
        extension.to_ascii_lowercase().as_str(),
        "m3u" | "m3u8" | "pls"
    ) {
        return read_playlists;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtered_folders_keep_matches_and_matching_container_contents() {
        let filter = TreeFilter::new(" FINAL fantasy ", "/music");
        assert!(filter.matches_path(Path::new("/music/NSFe/Final Fantasy III.rar/01.nsfe")));
        assert!(filter.matches_path(Path::new("/music/Final Fantasy/Disc 1/01.nsfe")));
        assert!(!filter.matches_path(Path::new("/music/NSFe/Zelda.rar/01.nsfe")));
        assert!(!filter.matches_path(Path::new("/music/Final/Fantasy/01.nsfe")));
        assert!(
            !TreeFilter::new("music", "/music").matches_path(Path::new("/music/other/song.flac"))
        );
        assert!(TreeFilter::default().matches_path(Path::new("/music/anything.flac")));
    }

    #[test]
    fn explicit_files_bypass_folder_preferences() {
        assert!(!include_discovered_file(
            Path::new("album.CUE"),
            false,
            true,
            false
        ));
        assert!(!include_discovered_file(
            Path::new("album.M3U"),
            true,
            false,
            false
        ));
        assert!(include_discovered_file(
            Path::new("album.CUE"),
            false,
            false,
            true
        ));
        assert!(include_discovered_file(
            Path::new("song.flac"),
            false,
            false,
            false
        ));
    }
}
