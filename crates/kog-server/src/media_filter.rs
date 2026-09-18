//! Small shared rules for library browsing: what to hide, and how an entry is
//! addressed.
//!
//! The locator scheme is load-bearing: stars written through the API must land
//! on the same keys the desktop app uses, or a track starred on a phone would
//! not appear starred on the desktop. It matches `radio_track_key` plus the
//! subsong fragment.

use std::path::Path;

use kog_core::db::{KIND_ARCHIVE, KIND_LOCAL, KIND_REMOTE, StoredEntry};

/// Skip metadata sidecars, dotfiles and macOS resource forks so the API never
/// offers a client something unplayable or meaningless.
pub fn is_hidden(path: &Path) -> bool {
    if kog_core::media_path::is_metadata(path) {
        return true;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    name.starts_with('.') || name == "__MACOSX"
}

/// The identity a track is stored under: `path` for local files and URLs,
/// `outer::member` for archive entries, with `#fragment` for subsongs.
pub fn locator_for(entry: &StoredEntry) -> Result<String, String> {
    if entry.path.trim().is_empty() {
        return Err("an entry path is required".to_owned());
    }
    let base = match entry.kind.as_str() {
        KIND_LOCAL | KIND_REMOTE => entry.path.clone(),
        KIND_ARCHIVE => {
            if entry.entry.trim().is_empty() {
                return Err("an archive entry needs a member name".to_owned());
            }
            format!("{}::{}", entry.path, entry.entry)
        }
        other => return Err(format!("unknown entry kind: {other}")),
    };
    Ok(match entry.fragment.as_deref() {
        Some(fragment) if !fragment.trim().is_empty() => {
            format!("{base}#{}", fragment.trim())
        }
        _ => base,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(kind: &str, path: &str, member: &str, fragment: Option<&str>) -> StoredEntry {
        StoredEntry {
            kind: kind.to_owned(),
            path: path.to_owned(),
            entry: member.to_owned(),
            fragment: fragment.map(str::to_owned),
        }
    }

    #[test]
    fn locators_match_the_desktop_star_keys() {
        assert_eq!(
            locator_for(&entry(KIND_LOCAL, "/music/a.flac", "", None)).unwrap(),
            "/music/a.flac"
        );
        assert_eq!(
            locator_for(&entry(KIND_LOCAL, "/music/album.cue", "", Some("3"))).unwrap(),
            "/music/album.cue#3",
            "a cue track stars independently of its neighbours"
        );
        assert_eq!(
            locator_for(&entry(KIND_ARCHIVE, "/music/pack.zip", "Disc/a.wav", None)).unwrap(),
            "/music/pack.zip::Disc/a.wav"
        );
        assert_eq!(
            locator_for(&entry(KIND_ARCHIVE, "/music/pack.zip", "a.wav", Some("2"))).unwrap(),
            "/music/pack.zip::a.wav#2"
        );
        assert_eq!(
            locator_for(&entry(KIND_REMOTE, "https://example.com/s.mp3", "", None)).unwrap(),
            "https://example.com/s.mp3"
        );
    }

    #[test]
    fn incomplete_entries_are_rejected() {
        assert!(locator_for(&entry(KIND_LOCAL, "  ", "", None)).is_err());
        assert!(locator_for(&entry(KIND_ARCHIVE, "/music/pack.zip", "", None)).is_err());
        assert!(locator_for(&entry("nonsense", "/music/a.flac", "", None)).is_err());
    }

    #[test]
    fn hidden_and_metadata_files_are_filtered() {
        assert!(is_hidden(&PathBuf::from("/music/._song.flac")));
        assert!(is_hidden(&PathBuf::from("/music/.hidden.flac")));
        assert!(is_hidden(&PathBuf::from("/music/__MACOSX/song.flac")));
        assert!(!is_hidden(&PathBuf::from("/music/song.flac")));
        assert!(!is_hidden(&PathBuf::from("/music/Album One/track.flac")));
    }
}
