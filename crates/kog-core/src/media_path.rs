//! OS metadata is not media, even when an AppleDouble file has an audio suffix.
//! Keep the equivalent native tree predicate in kog_media_path.h in sync.
use std::path::Path;

const METADATA_NAMES: [&[u8]; 11] = [
    b".DS_Store",
    b"__MACOSX",
    b".AppleDouble",
    b".LSOverride",
    b".Spotlight-V100",
    b".Trashes",
    b".fseventsd",
    b".TemporaryItems",
    b"Thumbs.db",
    b"ehthumbs.db",
    b"desktop.ini",
];

pub fn is_metadata(path: &Path) -> bool {
    // Allocation-free on purpose: the walk, browse and radio paths run this
    // per entry, and the lossy lowercase dance used to allocate twice per
    // path component — measurable across a million-file library.
    path.components().any(|part| {
        let bytes = part.as_os_str().as_encoded_bytes();
        bytes.starts_with(b"._")
            || bytes.eq_ignore_ascii_case(b"$RECYCLE.BIN")
            || bytes.eq_ignore_ascii_case(b"System Volume Information")
            || METADATA_NAMES
                .iter()
                .any(|candidate| bytes.eq_ignore_ascii_case(candidate))
    })
}

/// Whether one path component (a file name) is a metadata file, the
/// entry-level half of [`is_metadata`] for callers that hoist the ancestor
/// check out of a per-entry loop.
pub fn is_metadata_name(name: &std::ffi::OsStr) -> bool {
    is_metadata(std::path::Path::new(name))
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
