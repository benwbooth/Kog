use std::path::{Path, PathBuf};
use std::time::Duration;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey, Tag};

use crate::decoder::{DecoderRegistry, PlaybackSource};

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub source: PlaybackSource,
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub genre: String,
    pub composer: String,
    pub lyrics: String,
    pub year: Option<u32>,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub duration: Option<Duration>,
    /// Size of the playable file (the extracted member for an archive entry).
    pub file_size_bytes: Option<u64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bitrate: Option<u32>,
    pub bits_per_sample: Option<u8>,
    pub decoder_warning: Option<String>,
    pub codec: String,
    pub backend_id: String,
    pub search_text: String,
    /// Set once playback (or an explicit check) proves the file is gone.
    /// Grayed out in the pane and skipped by next/previous/auto-advance.
    pub missing: bool,
}

impl Track {
    pub fn from_source(source: PlaybackSource, decoders: &DecoderRegistry) -> Self {
        let mut fallback_title = source
            .path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Untitled")
            .to_owned();
        // Number the later songs of a multi-song file; the first (and a
        // file's only song) keeps the plain name.
        if let Some(subsong) = source.subsong.filter(|subsong| *subsong > 0) {
            fallback_title.push_str(&format!(" [{}]", subsong + 1));
        }
        let codec = source
            .path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_uppercase();
        let mut track = Self {
            source,
            title: fallback_title,
            codec,
            ..Self::default()
        };
        if !track.source.is_remote() {
            track.file_size_bytes = std::fs::metadata(&track.source.path)
                .ok()
                .map(|metadata| metadata.len());
        }

        let mut tagged_metadata_loaded = false;
        if !track.source.is_remote()
            && let Ok(tagged_file) = lofty::read_from_path(&track.source.path)
        {
            tagged_metadata_loaded = true;
            let properties = tagged_file.properties();
            track.duration = Some(properties.duration());
            track.sample_rate = properties.sample_rate();
            track.channels = properties.channels().map(u16::from);
            track.bitrate = properties.audio_bitrate();
            track.bits_per_sample = properties.bit_depth();

            if let Some(tag) = tagged_file
                .primary_tag()
                .or_else(|| tagged_file.first_tag())
            {
                if let Some(title) = tag.title() {
                    track.title = title.to_string();
                }
                track.artist = tag
                    .artist()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                track.album = tag
                    .album()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                track.album_artist = tag
                    .get_string(ItemKey::AlbumArtist)
                    .unwrap_or_default()
                    .to_owned();
                track.genre = tag
                    .genre()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                track.composer = tag
                    .get_string(ItemKey::Composer)
                    .unwrap_or_default()
                    .to_owned();
                track.lyrics = lyrics_from_tag(tag);
                track.year = tag.date().map(|date| u32::from(date.year));
                track.disc_number = tag.disk();
                track.track_number = tag.track();
            }
        }

        let selected_backend_id = decoders.selected_backend_id(&track.source);
        if tagged_metadata_loaded && selected_backend_id == Some("rodio-symphonia") {
            track.backend_id = "rodio-symphonia".to_owned();
        } else if let Ok((backend_id, properties)) = decoders.probe_with_backend(&track.source) {
            track.backend_id = backend_id.to_owned();
            track.duration = properties.duration.or(track.duration);
            track.sample_rate = properties.sample_rate.or(track.sample_rate);
            track.channels = properties.channels.or(track.channels);
            track.year = properties.year.or(track.year);
            track.bitrate = properties.bitrate.or(track.bitrate);
            track.bits_per_sample = properties.bits_per_sample.or(track.bits_per_sample);
            if let Some(codec) = properties.codec.filter(|value| !value.is_empty()) {
                track.codec = codec;
            }
            if let Some(title) = properties.title.filter(|value| !value.is_empty()) {
                track.title = title;
            }
            if let Some(artist) = properties.artist.filter(|value| !value.is_empty()) {
                track.artist = artist;
            }
            if let Some(album) = properties.album.filter(|value| !value.is_empty()) {
                track.album = album;
            }
            if let Some(genre) = properties.genre.filter(|value| !value.is_empty()) {
                track.genre = genre;
            }
            if let Some(lyrics) = properties.lyrics.filter(|value| !value.is_empty()) {
                track.lyrics = lyrics;
            }
            track.track_number = properties.track_number.or(track.track_number);
            track.disc_number = properties.disc_number.or(track.disc_number);
            track.decoder_warning = properties.warning;
        }

        if !track.source.is_remote()
            && crate::legacy_id3::looks_misdecoded(&[
                &track.title,
                &track.artist,
                &track.album,
                &track.album_artist,
                &track.composer,
                &track.genre,
            ])
            && let Some(corrected) = crate::legacy_id3::corrected_text(&track.source.path)
        {
            if let Some(value) = corrected.title {
                track.title = value;
            }
            if let Some(value) = corrected.artist {
                track.artist = value;
            }
            if let Some(value) = corrected.album {
                track.album = value;
            }
            if let Some(value) = corrected.album_artist {
                track.album_artist = value;
            }
            if let Some(value) = corrected.composer {
                track.composer = value;
            }
            if let Some(value) = corrected.genre {
                track.genre = value;
            }
        }

        if !title_is_informative(&track.title, &track.source) {
            if let Some(better) = title_from_filename(&track.source) {
                track.title = better;
            }
        } else if let Some(name) = filename_adding_to_short_title(&track.title, &track.source) {
            // "Theme" alone says little; "Theme · Shofixti" says which.
            track.title = format!("{} · {name}", track.title.trim());
        }
        if track.album.trim().is_empty() {
            if let Some(album) = album_from_folder(&track.source) {
                track.album = album;
            }
        }
        track.refresh_search_text();
        track
    }

    fn refresh_search_text(&mut self) {
        let source_filename = search_source_filename(&self.source);
        self.search_text = [
            self.title.as_str(),
            self.artist.as_str(),
            self.album_artist.as_str(),
            self.album.as_str(),
            self.genre.as_str(),
            self.composer.as_str(),
            source_filename.as_str(),
        ]
        .join("\n")
        .to_lowercase();
    }

    pub fn matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        crate::playback_order::sort::matches_query(&[&self.search_text], query)
    }

    pub fn duration_label(&self) -> String {
        duration_label(self.duration.unwrap_or_default())
    }
}

/// Whether a song's own title says which song it is. Trackers and MIDI
/// sequencers leave names like `Seq-1`, `untitled`, `A`, another file's name
/// or the same tool stamp (`WinJammer Demo`) on every song in a folder.
pub fn title_is_informative(title: &str, source: &PlaybackSource) -> bool {
    let title = title.trim();
    let lower = title.to_lowercase();
    let letters = title.chars().filter(|c| c.is_alphanumeric()).count();
    if letters < 2 {
        return false;
    }
    // The fallback is already the filename (perhaps with a subsong number).
    let stem = Path::new(&search_source_filename(source))
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if lower == stem || lower.starts_with(&format!("{stem} [")) {
        return true;
    }
    const GENERIC: &[&str] = &[
        "untitled", "untitled song", "no name", "noname", "unnamed", "new", "new song", "song", "track",
        "tune", "title", "music", "midi", "sequence", "default", "demo", "test", "a", "b", "x",
    ];
    if GENERIC.contains(&lower.as_str()) {
        return false;
    }
    // Numbered placeholders: "Seq-1", "Seq-1: GMD", "Track 3", "Sequence #2", "12".
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let placeholder = ["seq", "sequence", "track", "song", "pattern", "part", "tune", "midi", "untitled"];
    if words.first().is_some_and(|first| placeholder.contains(first))
        && words.get(1).is_some_and(|second| second.chars().all(|c| c.is_ascii_digit()))
        && words.len() <= 3
    {
        return false;
    }
    if words.iter().all(|word| word.chars().all(|c| c.is_ascii_digit())) {
        return false;
    }
    // Another file's name: "Jaffar3.mff", "fighting.roP".
    if let Some((name, extension)) = title.rsplit_once('.')
        && !name.is_empty()
        && !name.contains(' ')
        && (2..=4).contains(&extension.len())
        && extension.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return false;
    }
    const STAMPS: &[&str] = &["winjammer", "cakewalk", "created by", "generated by", "converted by", "sequenced by", "copyright", "(c)"];
    if STAMPS.iter().any(|stamp| lower.contains(stamp)) {
        return false;
    }
    !shared_by_sibling_midi(&lower, source)
}

/// A MIDI title that three or more files in the same folder carry is the
/// sequencer's, not the song's.
fn shared_by_sibling_midi(title: &str, source: &PlaybackSource) -> bool {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    const MIDI: &[&str] = &["mid", "midi", "rmi", "kar", "smf"];
    let is_midi = |path: &Path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| MIDI.iter().any(|midi| midi.eq_ignore_ascii_case(extension)))
    };
    if source.archive_origin.is_some() || source.is_remote() || !is_midi(&source.path) {
        return false;
    }
    let Some(folder) = source.path.parent() else { return false };
    static TITLES: OnceLock<Mutex<HashMap<PathBuf, HashMap<String, usize>>>> = OnceLock::new();
    let titles = TITLES.get_or_init(Default::default);
    let mut titles = titles.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if titles.len() > 256 {
        titles.clear();
    }
    let counts = titles.entry(folder.to_path_buf()).or_insert_with(|| {
        let mut counts = HashMap::new();
        let Ok(entries) = std::fs::read_dir(folder) else { return counts };
        for path in entries.flatten().map(|entry| entry.path()).filter(|path| is_midi(path)).take(64) {
            if let Some(title) = crate::decoder::read_standard_midi_subsong(&path, None)
                .ok()
                .and_then(|midi| midi.title)
            {
                *counts.entry(title.trim().to_lowercase()).or_insert(0) += 1;
            }
        }
        counts
    });
    counts.get(title).is_some_and(|count| *count >= 3)
}

/// For a short title, the cleaned filename when it names the song in words
/// the title lacks (and is itself not a placeholder like `TRACK2` or `M07`).
fn filename_adding_to_short_title(title: &str, source: &PlaybackSource) -> Option<String> {
    let words = |text: &str| -> Vec<String> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_lowercase)
            .collect()
    };
    let title_words = words(title);
    let short = title_words.len() <= 2 && title.trim().chars().count() <= 14;
    if !short {
        return None;
    }
    let name = title_from_filename(source)?;
    let name_words = words(&name);
    // `TRACK2`, `M07`, `SONG00`, `BOSS-05`: a short word and a number.
    let numbered = name_words.len() <= 2
        && name_words.iter().any(|word| word.chars().any(|c| c.is_ascii_digit()))
        && name_words.iter().all(|word| word.chars().filter(|c| c.is_alphabetic()).count() <= 6);
    let informative = !numbered
        && name.chars().filter(|c| c.is_alphabetic()).count() >= 4
        && title_is_informative(&name, &PlaybackSource::from_path(PathBuf::from("/")));
    let adds = name_words.iter().any(|word| word.chars().any(|c| c.is_alphabetic()) && !title_words.contains(word));
    let repeats = title_words.iter().all(|word| name_words.contains(word));
    (informative && adds && !repeats).then_some(name)
}

/// An album name from where the song lives: its archive, else its folder,
/// skipping folders that only sort versions or discs (`GM`, `MT`, `CD1`).
/// Rip folder names lose their tags: `Doom (1995-09)(iD)[SNES]` is "Doom",
/// `Street_Fighter_II_(CP_System)` is "Street Fighter II".
fn album_from_folder(source: &PlaybackSource) -> Option<String> {
    let start = match &source.archive_origin {
        Some(origin) => {
            let inner = Path::new(&origin.entry_name).parent().filter(|parent| !parent.as_os_str().is_empty());
            match inner {
                Some(inner) => inner.to_path_buf(),
                None => return clean_folder_name(origin.archive_path.file_stem()?.to_string_lossy().as_ref()),
            }
        }
        None => source.path.parent()?.to_path_buf(),
    };
    const SORTING: &[&str] = &[
        "gm", "gs", "mt", "mt32", "mt-32", "sc55", "midi", "mid", "music", "bgm", "audio", "sound", "sounds",
        "songs", "extract", "ver2", "files", "data",
    ];
    let mut folder = start.as_path();
    for _ in 0..3 {
        let name = folder.file_name()?.to_string_lossy().to_lowercase();
        let disc = ["cd", "disc", "disk", "side"]
            .iter()
            .any(|word| name.strip_prefix(word).is_some_and(|rest| rest.trim().chars().all(|c| c.is_ascii_digit() || c.is_alphabetic() && rest.trim().len() == 1)));
        if !SORTING.contains(&name.as_str()) && !disc {
            break;
        }
        folder = folder.parent()?;
    }
    clean_folder_name(folder.file_name()?.to_string_lossy().as_ref())
}

fn clean_folder_name(name: &str) -> Option<String> {
    let name = name.replace('_', " ");
    // Rip tags start at the first "(" or "[" after the name.
    let name = match name.find(['(', '[']) {
        Some(0) | None => name.as_str(),
        Some(at) => &name[..at],
    };
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = name.trim_end_matches([' ', '-', ',']).to_owned();
    (name.chars().filter(|c| c.is_alphanumeric()).count() >= 2).then_some(name)
}

/// The filename as a title: no extension, separators as spaces, and rip
/// prefixes such as `PCGame-COMMANDER_KEEN-` dropped. The subsong number
/// stays for files holding several songs.
fn title_from_filename(source: &PlaybackSource) -> Option<String> {
    let filename = search_source_filename(source);
    let stem = Path::new(&filename).file_stem()?.to_string_lossy().into_owned();
    let mut title = stem.replace('_', " ");
    if let Some(rest) = title.strip_prefix("PCGame-") {
        title = rest.split_once('-').map_or(rest, |(_, song)| song).to_owned();
    }
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        return None;
    }
    Some(match source.subsong {
        Some(subsong) if subsong > 0 => format!("{title} [{}]", subsong + 1),
        _ => title,
    })
}

fn search_source_filename(source: &PlaybackSource) -> String {
    let path = source
        .archive_origin
        .as_ref()
        .map(|origin| Path::new(&origin.entry_name))
        .unwrap_or(&source.path);
    path.file_name()
        .map(|filename| filename.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn lyrics_from_tag(tag: &Tag) -> String {
    for key in [ItemKey::UnsyncLyrics, ItemKey::Lyrics] {
        let values = tag
            .get_strings(key)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if !values.is_empty() {
            return values.join("\n");
        }
    }
    String::new()
}

pub fn duration_label(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

pub fn file_size_label(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = "B";
    for next in ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] {
        value /= 1024.0;
        unit = next;
        if value < 1024.0 {
            break;
        }
    }
    format!("{value:.1} {unit}")
}

pub fn canonical_path(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("resolving {}: {error}", path.display()))
}

#[cfg(any(test, feature = "test-util"))]
mod tests {

    #[test]
    fn sequencer_and_placeholder_titles_give_way_to_the_filename() {
        let source = |name: &str| PlaybackSource::from_path(PathBuf::from(format!("/nowhere/{name}")));
        for junk in ["Seq-1", "Seq-1: GMD", "untitled", "A", "New", "   ", "Jaffar3.mff", "fighting.roP", "WinJammer Demo", "Track 3", "12"] {
            assert!(!title_is_informative(junk, &source("song.mid")), "{junk} should not count as a title");
        }
        for good in ["Hyperspace Travel", "Shofixti's Theme", "Paavinkatu blues", "- One Must Fall! 1 -", "song"] {
            assert!(title_is_informative(good, &source("song.mid")), "{good} is a real title");
        }
        assert_eq!(filename_adding_to_short_title("Theme", &source("Shofixti.mid")).as_deref(), Some("Shofixti"));
        assert_eq!(filename_adding_to_short_title("Hyperspace Travel", &source("H-Space.mid")), None);
        assert_eq!(filename_adding_to_short_title("Death", &source("BOSS-05.MID")), None);
        assert_eq!(filename_adding_to_short_title("Diamondus", &source("Diamondus.s3m")), None);
        assert_eq!(filename_adding_to_short_title("Combat", &source("TRACK2.S3M")), None);
        assert_eq!(filename_adding_to_short_title("Shofixti's Theme", &source("Shofixti.mid")), None);
        assert_eq!(title_from_filename(&source("PCGame-COMMANDER_KEEN-fish_polka.mid")).as_deref(), Some("fish polka"));
        let at = |path: &str| PlaybackSource::from_path(PathBuf::from(path));
        assert_eq!(album_from_folder(&at("/m/gamemids/King's Quest 4/GM/01 - Introduction.MID")).as_deref(), Some("King's Quest 4"));
        assert_eq!(album_from_folder(&at("/m/SNES/Doom (1995-09)(iD)(Sculptured)[SNES]/02 At Doom's Gate.spc")).as_deref(), Some("Doom"));
        assert_eq!(album_from_folder(&at("/m/Arcade/Street_Fighter_II_-_The_World_Warrior_(CP_System)/01 Title.vgz")).as_deref(),
                   Some("Street Fighter II - The World Warrior"));
        assert_eq!(album_from_folder(&at("/m/Final Fantasy 10 OST/Disc 1/15.mp3")).as_deref(), Some("Final Fantasy 10 OST"));
        assert_eq!(title_from_filename(&source("02 - Debut Of The Chron-O-John.mid")).as_deref(), Some("02 - Debut Of The Chron-O-John"));
    }
    use super::*;
    use lofty::tag::TagType;

    #[test]
    fn duration_labels_match_music_player_conventions() {
        assert_eq!(duration_label(Duration::from_secs(0)), "0:00");
        assert_eq!(duration_label(Duration::from_secs(185)), "3:05");
        assert_eq!(duration_label(Duration::from_secs(3_661)), "1:01:01");
    }

    #[test]
    fn file_size_labels_use_binary_units() {
        assert_eq!(file_size_label(0), "0 B");
        assert_eq!(file_size_label(1023), "1023 B");
        assert_eq!(file_size_label(1024), "1.0 KiB");
        assert_eq!(file_size_label(1_572_864), "1.5 MiB");
    }

    #[test]
    fn lyrics_follow_cogs_unsynchronized_then_generic_fallback() {
        let mut id3 = Tag::new(TagType::Id3v2);
        assert!(id3.insert_text(ItemKey::UnsyncLyrics, "First line\nSecond line".to_owned()));
        assert_eq!(lyrics_from_tag(&id3), "First line\nSecond line");

        let mut vorbis = Tag::new(TagType::VorbisComments);
        assert!(vorbis.insert_text(ItemKey::Lyrics, "Generic lyrics".to_owned()));
        assert_eq!(lyrics_from_tag(&vorbis), "Generic lyrics");

        assert_eq!(lyrics_from_tag(&Tag::new(TagType::Id3v2)), "");
    }

    #[test]
    fn playlist_search_uses_one_normalized_metadata_cache() {
        let mut track = Track {
            title: "Neon Skyline".to_owned(),
            artist: "Blue-Green Ensemble".to_owned(),
            source: PlaybackSource::from_path(PathBuf::from("/music/Final Theme.FLAC")),
            ..Track::default()
        };
        track.refresh_search_text();

        assert!(track.matches("neon skyline"));
        assert!(track.matches("blue-green"));
        assert!(track.matches("final theme.flac"));
        assert!(!track.matches("missing"));
    }

    #[test]
    fn playlist_search_filters_on_the_first_character_without_matching_parent_paths() {
        let mut matching = Track {
            title: "Aerial".to_owned(),
            source: PlaybackSource::from_path(PathBuf::from("/var/media/first.ogg")),
            ..Track::default()
        };
        let mut non_matching = Track {
            title: "Bore".to_owned(),
            source: PlaybackSource::from_path(PathBuf::from("/var/media/second.ogg")),
            ..Track::default()
        };
        matching.refresh_search_text();
        non_matching.refresh_search_text();

        let visible_rows = [matching, non_matching]
            .iter()
            .enumerate()
            .filter(|(_, track)| track.matches("a"))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        assert_eq!(visible_rows, [0]);
    }
}
