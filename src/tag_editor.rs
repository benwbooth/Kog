use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{Picture, PictureType};
use lofty::tag::{Accessor, ItemKey, Tag, TagExt};
use serde_json::{Map, Value, json};

use crate::decoder::PlaybackSource;

const MAX_SELECTION: usize = 512;
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_FIELD_BYTES: usize = 64 * 1024;
const MAX_LYRICS_BYTES: usize = 1024 * 1024;
pub const MAX_ARTWORK_BYTES: u64 = 16 * 1024 * 1024;

const TEXT_FIELDS: &[(&str, ItemKey)] = &[
    ("title", ItemKey::TrackTitle),
    ("artist", ItemKey::TrackArtist),
    ("albumArtist", ItemKey::AlbumArtist),
    ("album", ItemKey::AlbumTitle),
    ("composer", ItemKey::Composer),
    ("genre", ItemKey::Genre),
    ("year", ItemKey::RecordingDate),
    ("trackNumber", ItemKey::TrackNumber),
    ("trackTotal", ItemKey::TrackTotal),
    ("discNumber", ItemKey::DiscNumber),
    ("discTotal", ItemKey::DiscTotal),
    ("comment", ItemKey::Comment),
    ("lyrics", ItemKey::UnsyncLyrics),
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagEdits {
    fields: Vec<(&'static str, ItemKey, String)>,
    artwork: ArtworkEdit,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum ArtworkEdit {
    #[default]
    Keep,
    Remove,
    Replace(PathBuf),
}

#[derive(Debug, Default)]
pub struct WriteOutcome {
    pub updated_paths: Vec<PathBuf>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TagValues {
    title: String,
    artist: String,
    album_artist: String,
    album: String,
    composer: String,
    genre: String,
    year: String,
    track_number: String,
    track_total: String,
    disc_number: String,
    disc_total: String,
    comment: String,
    lyrics: String,
}

impl TagValues {
    fn from_tag(tag: Option<&Tag>) -> Self {
        let Some(tag) = tag else {
            return Self::default();
        };
        Self {
            title: accessor_text(tag.title()),
            artist: accessor_text(tag.artist()),
            album_artist: tag
                .get_string(ItemKey::AlbumArtist)
                .unwrap_or_default()
                .to_owned(),
            album: accessor_text(tag.album()),
            composer: tag
                .get_string(ItemKey::Composer)
                .unwrap_or_default()
                .to_owned(),
            genre: accessor_text(tag.genre()),
            year: tag.date().map(|date| date.to_string()).unwrap_or_default(),
            track_number: optional_number(tag.track()),
            track_total: optional_number(tag.track_total()),
            disc_number: optional_number(tag.disk()),
            disc_total: optional_number(tag.disk_total()),
            comment: tag
                .get_strings(ItemKey::Comment)
                .collect::<Vec<_>>()
                .join("\n"),
            lyrics: lyrics_from_tag(tag),
        }
    }

    fn values(&self) -> [(&'static str, &str); 13] {
        [
            ("title", &self.title),
            ("artist", &self.artist),
            ("albumArtist", &self.album_artist),
            ("album", &self.album),
            ("composer", &self.composer),
            ("genre", &self.genre),
            ("year", &self.year),
            ("trackNumber", &self.track_number),
            ("trackTotal", &self.track_total),
            ("discNumber", &self.disc_number),
            ("discTotal", &self.disc_total),
            ("comment", &self.comment),
            ("lyrics", &self.lyrics),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArtworkValue {
    data: Vec<u8>,
    mime: String,
    description: String,
    picture_type: PictureType,
}

pub fn snapshot_json(sources: &[PlaybackSource]) -> Result<Value, String> {
    let paths = editable_paths(sources)?;
    let mut values = Vec::with_capacity(paths.len());
    let mut artwork = Vec::with_capacity(paths.len());

    for path in &paths {
        let tagged = lofty::read_from_path(path)
            .map_err(|error| format!("Reading tags from {}: {error}", path.display()))?;
        let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
        values.push(TagValues::from_tag(tag));
        artwork.push(tag.and_then(selected_artwork));
    }

    let mut fields = Map::new();
    for (name, first_value) in values[0].values() {
        let mixed = values
            .iter()
            .skip(1)
            .any(|value| field_value(value, name) != first_value);
        fields.insert(
            name.to_owned(),
            json!({
                "value": if mixed { "" } else { first_value },
                "mixed": mixed,
            }),
        );
    }

    let artwork_json = artwork_summary(&artwork);
    let selection_count = paths.len();
    Ok(json!({
        "ok": true,
        "selectionCount": selection_count,
        "summary": if selection_count == 1 {
            paths[0].file_name().and_then(|name| name.to_str()).unwrap_or("1 track").to_owned()
        } else {
            format!("{selection_count} tracks")
        },
        "location": if selection_count == 1 {
            paths[0].to_string_lossy().into_owned()
        } else {
            common_parent_label(&paths)
        },
        "fields": fields,
        "artwork": artwork_json,
    }))
}

pub fn parse_edits(value: &str) -> Result<TagEdits, String> {
    if value.len() > MAX_REQUEST_BYTES {
        return Err("Tag edit request exceeds Kog's 2 MiB safety limit".to_owned());
    }
    let request: Value = serde_json::from_str(value)
        .map_err(|error| format!("The tag edit request is invalid: {error}"))?;
    let object = request
        .as_object()
        .ok_or_else(|| "The tag edit request must be an object".to_owned())?;
    for key in object.keys() {
        if !matches!(key.as_str(), "fields" | "artwork") {
            return Err(format!("Unknown tag edit section: {key}"));
        }
    }

    let mut edits = TagEdits::default();
    if let Some(fields) = object.get("fields") {
        let fields = fields
            .as_object()
            .ok_or_else(|| "Tag fields must be an object".to_owned())?;
        for (name, value) in fields {
            let Some((canonical, key)) = TEXT_FIELDS
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .copied()
            else {
                return Err(format!("Unknown tag field: {name}"));
            };
            let value = value
                .as_str()
                .ok_or_else(|| format!("Tag field {name} must be text"))?;
            let limit = if name == "lyrics" {
                MAX_LYRICS_BYTES
            } else {
                MAX_FIELD_BYTES
            };
            if value.len() > limit {
                return Err(format!("Tag field {name} exceeds its safety limit"));
            }
            if value.contains('\0') {
                return Err(format!("Tag field {name} contains a NUL character"));
            }
            validate_numeric_field(name, value)?;
            edits.fields.push((canonical, key, value.to_owned()));
        }
    }

    if let Some(artwork) = object.get("artwork") {
        let artwork = artwork
            .as_object()
            .ok_or_else(|| "Artwork edit must be an object".to_owned())?;
        let action = artwork
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| "Artwork edit is missing an action".to_owned())?;
        edits.artwork = match action {
            "keep" => ArtworkEdit::Keep,
            "remove" => ArtworkEdit::Remove,
            "replace" => {
                let path = artwork
                    .get("path")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "Replacement artwork is missing its path".to_owned())?;
                ArtworkEdit::Replace(PathBuf::from(path))
            }
            _ => return Err(format!("Unknown artwork action: {action}")),
        };
    }
    if edits.fields.is_empty() && edits.artwork == ArtworkEdit::Keep {
        return Err("No tag changes were supplied".to_owned());
    }
    Ok(edits)
}

pub fn write_tags(sources: &[PlaybackSource], edits: &TagEdits) -> WriteOutcome {
    let paths = match editable_paths(sources) {
        Ok(paths) => paths,
        Err(error) => {
            return WriteOutcome {
                error: Some(error),
                ..WriteOutcome::default()
            };
        }
    };
    let replacement = match &edits.artwork {
        ArtworkEdit::Replace(path) => match read_artwork(path) {
            Ok(picture) => Some(picture),
            Err(error) => {
                return WriteOutcome {
                    error: Some(error),
                    ..WriteOutcome::default()
                };
            }
        },
        _ => None,
    };

    let mut prepared = Vec::with_capacity(paths.len());
    for path in &paths {
        match prepare_tag(path, edits, replacement.as_ref()) {
            Ok(tag) => prepared.push((path.clone(), tag)),
            Err(error) => {
                return WriteOutcome {
                    error: Some(error),
                    ..WriteOutcome::default()
                };
            }
        }
    }

    let mut outcome = WriteOutcome::default();
    for (path, tag) in prepared {
        if let Err(error) = tag.save_to_path(&path, WriteOptions::default()) {
            outcome.error = Some(format!(
                "Writing tags to {} failed after {} file{}: {error}",
                path.display(),
                outcome.updated_paths.len(),
                if outcome.updated_paths.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ));
            break;
        }
        outcome.updated_paths.push(path);
    }
    outcome
}

pub fn artwork_file_json(path: &Path) -> Result<Value, String> {
    let picture = read_artwork(path)?;
    Ok(json!({
        "ok": true,
        "path": path.to_string_lossy(),
        "uri": picture_data_uri(&picture),
        "description": artwork_description(&picture),
    }))
}

fn editable_paths(sources: &[PlaybackSource]) -> Result<Vec<PathBuf>, String> {
    if sources.is_empty() {
        return Err("Select at least one playlist track to edit tags".to_owned());
    }
    if sources.len() > MAX_SELECTION {
        return Err(format!(
            "Select no more than {MAX_SELECTION} tracks for one tag edit"
        ));
    }
    let mut paths = Vec::with_capacity(sources.len());
    let mut seen = HashSet::with_capacity(sources.len());
    for source in sources {
        if source.is_remote() {
            return Err("Tags cannot be written to remote streams".to_owned());
        }
        if source.archive_origin.is_some() {
            return Err("Tags cannot be written through an archive member".to_owned());
        }
        if source.subsong.is_some() {
            return Err("Tags cannot be written to CUE tracks or subsongs".to_owned());
        }
        let path = source
            .path
            .canonicalize()
            .map_err(|error| format!("Resolving {}: {error}", source.path.display()))?;
        if !path.is_file() {
            return Err(format!("{} is not a regular file", path.display()));
        }
        if !seen.insert(path.clone()) {
            continue;
        }
        paths.push(path);
    }
    if paths.is_empty() {
        return Err("The selection contains no unique editable files".to_owned());
    }
    Ok(paths)
}

fn prepare_tag(
    path: &Path,
    edits: &TagEdits,
    replacement: Option<&Picture>,
) -> Result<Tag, String> {
    let tagged = lofty::read_from_path(path)
        .map_err(|error| format!("Reading tags from {}: {error}", path.display()))?;
    let tag_type = tagged.primary_tag_type();
    if !tagged.tag_support(tag_type).is_writable() {
        return Err(format!("{} does not support writable tags", path.display()));
    }
    let mut tag = tagged
        .primary_tag()
        .cloned()
        .or_else(|| tagged.first_tag().cloned())
        .unwrap_or_else(|| Tag::new(tag_type));
    if tag.tag_type() != tag_type {
        tag.re_map(tag_type);
    }

    for (name, key, value) in &edits.fields {
        clear_equivalent_keys(&mut tag, name, *key);
        if !value.is_empty() && !tag.insert_text(*key, value.clone()) {
            return Err(format!(
                "{} cannot store the {name} field in its primary tag",
                path.display()
            ));
        }
    }
    match edits.artwork {
        ArtworkEdit::Keep => {}
        ArtworkEdit::Remove => remove_selected_artwork(&mut tag),
        ArtworkEdit::Replace(_) => {
            remove_selected_artwork(&mut tag);
            tag.push_picture(replacement.expect("replacement was validated").clone());
        }
    }
    Ok(tag)
}

fn clear_equivalent_keys(tag: &mut Tag, name: &str, key: ItemKey) {
    tag.remove_key(key);
    if name == "year" {
        tag.remove_key(ItemKey::Year);
    } else if name == "lyrics" {
        tag.remove_key(ItemKey::Lyrics);
    }
}

fn read_artwork(path: &Path) -> Result<Picture, String> {
    let metadata = path.metadata().map_err(|error| {
        format!(
            "Reading artwork information from {}: {error}",
            path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!("{} is not a regular image file", path.display()));
    }
    if metadata.len() > MAX_ARTWORK_BYTES {
        return Err(format!(
            "Artwork exceeds Kog's {} MiB safety limit",
            MAX_ARTWORK_BYTES / (1024 * 1024)
        ));
    }
    let file =
        File::open(path).map_err(|error| format!("Opening artwork {}: {error}", path.display()))?;
    let mut bounded = BufReader::new(file).take(MAX_ARTWORK_BYTES + 1);
    let mut picture = Picture::from_reader(&mut bounded)
        .map_err(|error| format!("{} is not a supported image: {error}", path.display()))?;
    if picture.data().len() as u64 > MAX_ARTWORK_BYTES {
        return Err(format!(
            "Artwork exceeds Kog's {} MiB safety limit",
            MAX_ARTWORK_BYTES / (1024 * 1024)
        ));
    }
    picture.set_pic_type(PictureType::CoverFront);
    Ok(picture)
}

fn selected_artwork(tag: &Tag) -> Option<ArtworkValue> {
    let picture = tag
        .pictures()
        .iter()
        .find(|picture| picture.pic_type() == PictureType::CoverFront)
        .or_else(|| tag.pictures().first())?;
    Some(ArtworkValue {
        data: picture.data().to_vec(),
        mime: picture
            .mime_type()
            .map(ToString::to_string)
            .unwrap_or_else(|| "application/octet-stream".to_owned()),
        description: picture.description().unwrap_or_default().to_owned(),
        picture_type: picture.pic_type(),
    })
}

fn remove_selected_artwork(tag: &mut Tag) {
    let selected = tag
        .pictures()
        .iter()
        .position(|picture| picture.pic_type() == PictureType::CoverFront)
        .or_else(|| (!tag.pictures().is_empty()).then_some(0));
    if let Some(index) = selected {
        tag.remove_picture(index);
    }
}

fn artwork_summary(values: &[Option<ArtworkValue>]) -> Value {
    let first = &values[0];
    let mixed = values.iter().skip(1).any(|value| value != first);
    if mixed {
        return json!({
            "state": "mixed",
            "uri": "",
            "description": "Different artwork across the selection",
            "oversized": false,
        });
    }
    let Some(artwork) = first else {
        return json!({
            "state": "none",
            "uri": "",
            "description": "No embedded artwork",
            "oversized": false,
        });
    };
    let oversized = artwork.data.len() as u64 > MAX_ARTWORK_BYTES;
    json!({
        "state": "same",
        "uri": if oversized { String::new() } else {
            format!("data:{};base64,{}", artwork.mime, BASE64.encode(&artwork.data))
        },
        "description": if oversized {
            format!("Embedded artwork is larger than {} MiB", MAX_ARTWORK_BYTES / (1024 * 1024))
        } else {
            format!("{} • {}", artwork.mime, byte_size_label(artwork.data.len()))
        },
        "oversized": oversized,
    })
}

fn picture_data_uri(picture: &Picture) -> String {
    let mime = picture
        .mime_type()
        .map(ToString::to_string)
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    format!("data:{mime};base64,{}", BASE64.encode(picture.data()))
}

fn artwork_description(picture: &Picture) -> String {
    let mime = picture
        .mime_type()
        .map(ToString::to_string)
        .unwrap_or_else(|| "image".to_owned());
    format!("{mime} • {}", byte_size_label(picture.data().len()))
}

fn byte_size_label(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} bytes")
    }
}

fn field_value<'a>(values: &'a TagValues, name: &str) -> &'a str {
    values
        .values()
        .into_iter()
        .find_map(|(candidate, value)| (candidate == name).then_some(value))
        .expect("field name originated from TagValues::values")
}

fn validate_numeric_field(name: &str, value: &str) -> Result<(), String> {
    if !matches!(
        name,
        "trackNumber" | "trackTotal" | "discNumber" | "discTotal"
    ) || value.is_empty()
    {
        return Ok(());
    }
    let number = value
        .parse::<u32>()
        .map_err(|_| format!("{name} must be a whole number"))?;
    if number == 0 {
        return Err(format!("{name} must be greater than zero"));
    }
    Ok(())
}

fn accessor_text(value: Option<std::borrow::Cow<'_, str>>) -> String {
    value.map(|value| value.into_owned()).unwrap_or_default()
}

fn optional_number(value: Option<u32>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
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

fn common_parent_label(paths: &[PathBuf]) -> String {
    let Some(parent) = paths[0].parent() else {
        return "Multiple locations".to_owned();
    };
    if paths.iter().all(|path| path.parent() == Some(parent)) {
        parent.to_string_lossy().into_owned()
    } else {
        "Multiple locations".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use lofty::file::TaggedFileExt;
    use lofty::tag::TagType;

    use super::*;

    fn write_wav(path: &Path) {
        let sample_rate = 8_000_u32;
        let samples = 800_u32;
        let data_size = samples * 2;
        let mut file = File::create(path).unwrap();
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36 + data_size).to_le_bytes()).unwrap();
        file.write_all(b"WAVEfmt ").unwrap();
        file.write_all(&16_u32.to_le_bytes()).unwrap();
        file.write_all(&1_u16.to_le_bytes()).unwrap();
        file.write_all(&1_u16.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        file.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
        file.write_all(&2_u16.to_le_bytes()).unwrap();
        file.write_all(&16_u16.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&data_size.to_le_bytes()).unwrap();
        for _ in 0..samples {
            file.write_all(&0_i16.to_le_bytes()).unwrap();
        }
    }

    fn source(path: &Path) -> PlaybackSource {
        PlaybackSource::from_path(path.to_owned())
    }

    fn seed_title(path: &Path, title: &str, artist: &str) {
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title(title.to_owned());
        tag.set_artist(artist.to_owned());
        tag.save_to_path(path, WriteOptions::default()).unwrap();
    }

    #[test]
    fn snapshot_marks_only_differing_multi_track_fields_as_mixed() {
        let fixture = tempfile::tempdir().unwrap();
        let first = fixture.path().join("first.wav");
        let second = fixture.path().join("second.wav");
        write_wav(&first);
        write_wav(&second);
        seed_title(&first, "First", "Shared artist");
        seed_title(&second, "Second", "Shared artist");

        let snapshot = snapshot_json(&[source(&first), source(&second)]).unwrap();
        assert_eq!(snapshot["fields"]["title"]["mixed"], true);
        assert_eq!(snapshot["fields"]["title"]["value"], "");
        assert_eq!(snapshot["fields"]["artist"]["mixed"], false);
        assert_eq!(snapshot["fields"]["artist"]["value"], "Shared artist");
    }

    #[test]
    fn partial_multi_track_edit_preserves_mixed_fields_and_reloads_written_tags() {
        let fixture = tempfile::tempdir().unwrap();
        let first = fixture.path().join("first.wav");
        let second = fixture.path().join("second.wav");
        write_wav(&first);
        write_wav(&second);
        seed_title(&first, "First", "Artist A");
        seed_title(&second, "Second", "Artist B");
        let sources = [source(&first), source(&second)];
        let edits = parse_edits(r#"{"fields":{"album":"One album"}}"#).unwrap();

        let outcome = write_tags(&sources, &edits);
        assert_eq!(outcome.updated_paths.len(), 2);
        assert!(outcome.error.is_none());

        let first_tags = lofty::read_from_path(&first).unwrap();
        let second_tags = lofty::read_from_path(&second).unwrap();
        let first_tag = first_tags.primary_tag().unwrap();
        let second_tag = second_tags.primary_tag().unwrap();
        assert_eq!(first_tag.title().as_deref(), Some("First"));
        assert_eq!(second_tag.title().as_deref(), Some("Second"));
        assert_eq!(first_tag.artist().as_deref(), Some("Artist A"));
        assert_eq!(second_tag.artist().as_deref(), Some("Artist B"));
        assert_eq!(first_tag.album().as_deref(), Some("One album"));
        assert_eq!(second_tag.album().as_deref(), Some("One album"));
    }

    #[test]
    fn empty_values_remove_fields_and_bad_requests_fail_closed() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("song.wav");
        write_wav(&path);
        seed_title(&path, "Old title", "Artist");
        let edits = parse_edits(r#"{"fields":{"title":"","trackNumber":"4"}}"#).unwrap();
        let outcome = write_tags(&[source(&path)], &edits);
        assert!(outcome.error.is_none());

        let tagged = lofty::read_from_path(path).unwrap();
        let tag = tagged.primary_tag().unwrap();
        assert!(tag.title().is_none());
        assert_eq!(tag.track(), Some(4));

        assert!(parse_edits(r#"{"fields":{"unknown":"value"}}"#).is_err());
        assert!(parse_edits(r#"{"fields":{"trackNumber":"zero"}}"#).is_err());
        assert!(parse_edits(r#"{"fields":{}}"#).is_err());
    }

    #[test]
    fn remote_archive_and_subsong_sources_are_never_writable() {
        let remote = PlaybackSource::from_remote_url(
            url::Url::parse("https://example.invalid/song.flac").unwrap(),
        );
        assert!(snapshot_json(&[remote]).is_err());

        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("song.wav");
        write_wav(&path);
        let mut subsong = source(&path);
        subsong.subsong = Some(0);
        assert!(snapshot_json(&[subsong]).is_err());

        let mut archived = source(&path);
        archived.set_archive_origin(fixture.path().join("songs.zip"), "song.wav".to_owned());
        assert!(snapshot_json(&[archived]).is_err());
    }

    #[test]
    fn artwork_replace_preview_and_remove_round_trip() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("song.wav");
        let artwork = fixture.path().join("cover.png");
        write_wav(&path);
        std::fs::write(
            &artwork,
            BASE64
                .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
                .unwrap(),
        )
        .unwrap();
        let track_source = source(&path);
        let replace = parse_edits(&format!(
            r#"{{"artwork":{{"action":"replace","path":{}}}}}"#,
            serde_json::to_string(&artwork.to_string_lossy()).unwrap()
        ))
        .unwrap();

        let outcome = write_tags(std::slice::from_ref(&track_source), &replace);
        assert!(outcome.error.is_none());
        let tagged = lofty::read_from_path(&path).unwrap();
        let picture = &tagged.primary_tag().unwrap().pictures()[0];
        assert_eq!(picture.pic_type(), PictureType::CoverFront);
        assert!(picture.data().starts_with(b"\x89PNG\r\n\x1a\n"));
        let snapshot = snapshot_json(std::slice::from_ref(&track_source)).unwrap();
        assert_eq!(snapshot["artwork"]["state"], "same");
        assert!(
            snapshot["artwork"]["uri"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );

        let remove = parse_edits(r#"{"artwork":{"action":"remove"}}"#).unwrap();
        let outcome = write_tags(&[track_source], &remove);
        assert!(outcome.error.is_none());
        let tagged = lofty::read_from_path(&path).unwrap();
        assert!(
            tagged
                .primary_tag()
                .is_none_or(|tag| tag.pictures().is_empty())
        );
        let snapshot = snapshot_json(&[source(&path)]).unwrap();
        assert_eq!(snapshot["artwork"]["state"], "none");
    }

    #[test]
    fn artwork_and_selection_limits_fail_before_expensive_work() {
        let fixture = tempfile::tempdir().unwrap();
        let oversized = fixture.path().join("oversized.png");
        File::create(&oversized)
            .unwrap()
            .set_len(MAX_ARTWORK_BYTES + 1)
            .unwrap();
        assert!(artwork_file_json(&oversized).is_err());

        let missing = fixture.path().join("missing.wav");
        let sources = vec![source(&missing); MAX_SELECTION + 1];
        let error = snapshot_json(&sources).unwrap_err();
        assert!(error.contains("no more than"));
    }

    #[test]
    fn malformed_audio_is_rejected_without_writing() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("broken.wav");
        std::fs::write(&path, b"not a wave file").unwrap();
        assert!(snapshot_json(&[source(&path)]).is_err());
        let edits = parse_edits(r#"{"fields":{"title":"New"}}"#).unwrap();
        let outcome = write_tags(&[source(&path)], &edits);
        assert!(outcome.updated_paths.is_empty());
        assert!(outcome.error.is_some());
        assert_eq!(std::fs::read(path).unwrap(), b"not a wave file");
    }
}
