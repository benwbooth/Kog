//! OS metadata is not media, even when an AppleDouble file has an audio suffix.
//! Keep the equivalent native tree predicate in kog_media_path.h in sync.
use std::path::Path;

pub fn is_metadata(path: &Path) -> bool {
    path.components().any(|part| {
        let name = part.as_os_str().to_string_lossy();
        name.starts_with("._")
            || matches!(
                name.to_ascii_lowercase().as_str(),
                ".ds_store"
                    | "__macosx"
                    | ".appledouble"
                    | ".lsoverride"
                    | ".spotlight-v100"
                    | ".trashes"
                    | ".fseventsd"
                    | ".temporaryitems"
                    | "thumbs.db"
                    | "ehthumbs.db"
                    | "desktop.ini"
                    | "$recycle.bin"
                    | "system volume information"
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skips_metadata_and_descendants_without_hiding_other_dotfiles() {
        for path in [
            "._song.flac",
            "Album/._song.mp3",
            "__MACOSX/Album/song.flac",
            "Album/.AppleDouble/song",
            "Album/Thumbs.db",
            "Album/DESKTOP.INI",
            ".Spotlight-V100/index",
            "$RECYCLE.BIN/song.mp3",
        ] {
            assert!(is_metadata(Path::new(path)), "{path}");
        }
        for path in [
            "Album/song.flac",
            ".music/song.mp3",
            "Album/.song.flac",
            "Album/Thumbs.db.flac",
            "MACOSX/song.mp3",
        ] {
            assert!(!is_metadata(Path::new(path)), "{path}");
        }
    }
}
