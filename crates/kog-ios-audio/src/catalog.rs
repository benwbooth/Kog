//! Native library navigation and expansion through the shared audio registry.
//! URLs for members retain the outer archive and nested path, so extraction
//! workspaces never leak into a persisted iPhone queue.

use std::collections::HashSet;
use std::ffi::{CStr, CString, c_char};
use std::path::{Path, PathBuf};
use std::ptr;

use kog_audio::archive;
use kog_audio::cover_art;
use kog_audio::decoder::{DecoderRegistry, DecoderSettings};
use kog_audio::track::Track as AudioTrack;
use serde_json::{Value, json};

use crate::error_to_buffer;

fn input_path(input: *const c_char) -> Result<PathBuf, String> {
    if input.is_null() {
        return Err("Missing library path".to_owned());
    }
    let path = unsafe { CStr::from_ptr(input) }
        .to_str()
        .map_err(|_| "Library path is not UTF-8".to_owned())?;
    Ok(PathBuf::from(path))
}

fn hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| {
            name.starts_with('.')
                || name == "__MACOSX"
                || kog_core::media_path::is_metadata_name(std::ffi::OsStr::new(name))
        })
}

fn within(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("reading {}: {error}", path.display()))?;
    if !path.starts_with(root) {
        return Err("That file is outside Kog's imported music".to_owned());
    }
    Ok(path)
}

fn archive_listing(
    root: &Path,
    path: &Path,
    extensions: &HashSet<String>,
) -> Result<Value, String> {
    let location = archive::tree_location(path)?;
    let (archive_path, entry) = match location {
        Some(location) if location.directory => (location.archive, location.entry),
        Some(_) => return Err("An archive file is not a directory".to_owned()),
        None => (path.to_path_buf(), String::new()),
    };
    let archive_path = within(root, &archive_path)?;
    let resolved =
        archive::resolve_archive_chain(&archive_path, &entry, &archive::nested_cache_dir())?;
    let members = archive::list_archive_names(&resolved.container)?;
    let listed = archive::browse_members(&members, &resolved.leaf, extensions);
    let full = |relative: &str| {
        if resolved.prefix.is_empty() {
            relative.to_owned()
        } else {
            format!("{}/{}", resolved.prefix, relative)
        }
    };
    let mut directories = Vec::new();
    for name in listed.directories {
        let relative = if resolved.leaf.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", resolved.leaf, name)
        };
        directories.push(json!({
            "name": name,
            "path": archive::member_url(&archive_path, &full(&relative), true).to_string_lossy(),
        }));
    }
    let mut files = Vec::new();
    for file in listed.files {
        let member = full(&file.entry);
        if archive::is_path(Path::new(&file.name)) {
            directories.push(json!({
                "name": file.name,
                "path": archive::member_url(&archive_path, &member, true).to_string_lossy(),
            }));
        } else {
            files.push(json!({
                "kind": "device",
                "name": file.name,
                "path": archive::member_url(&archive_path, &member, false).to_string_lossy(),
                "entry": member,
            }));
        }
    }
    directories.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let parent = match entry.rsplit_once('/') {
        Some((parent, _)) => archive::member_url(&archive_path, parent, true)
            .to_string_lossy()
            .into_owned(),
        None if !entry.is_empty() => archive_path.to_string_lossy().into_owned(),
        None => archive_path
            .parent()
            .unwrap_or(root)
            .to_string_lossy()
            .into_owned(),
    };
    Ok(json!({
        "path": path.to_string_lossy(),
        "parent": parent,
        "directories": directories,
        "files": files,
    }))
}

fn browse(root: &Path, path: &Path) -> Result<Value, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("reading imported music: {error}"))?;
    let registry = DecoderRegistry::new(DecoderSettings::default());
    let extensions = registry.audio_extensions();
    if archive::is_tree_location(path) || archive::is_path(path) && path.is_file() {
        return archive_listing(&root, path, &extensions);
    }
    let directory = within(&root, path)?;
    if !directory.is_dir() {
        return Err(format!("{} is not a folder", directory.display()));
    }
    let mut directories = Vec::new();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&directory)
        .map_err(|error| format!("listing {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if hidden(&path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() || archive::is_path(&path) {
            directories.push(json!({ "name": name, "path": path.to_string_lossy() }));
        } else if path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| extensions.contains(&ext.to_ascii_lowercase()))
        {
            files.push(json!({ "kind": "device", "name": name, "path": path.to_string_lossy() }));
        }
    }
    directories.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    files.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(json!({
        "path": directory.to_string_lossy(),
        "parent": if directory == root { String::new() } else { directory.parent().unwrap_or(&root).to_string_lossy().into_owned() },
        "directories": directories,
        "files": files,
    }))
}

fn expand(path: &Path) -> Result<Value, String> {
    let registry = DecoderRegistry::new(DecoderSettings::default());
    let mut paths = vec![path.to_path_buf()];
    let mut candidates = Vec::new();
    while let Some(path) = paths.pop() {
        if path.is_dir() {
            let entries = std::fs::read_dir(&path)
                .map_err(|error| format!("listing {}: {error}", path.display()))?;
            let mut children = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| !hidden(path))
                .collect::<Vec<_>>();
            children.sort();
            paths.extend(children.into_iter().rev());
        } else if registry.accepts_path(&path) {
            candidates.push(path);
        }
        if paths.len() + candidates.len() > 16_384 {
            return Err("An imported folder contains more than 16,384 playable entries".to_owned());
        }
    }
    let mut tracks = Vec::new();
    for path in candidates {
        let expanded = registry.expand_detailed(path)?;
        for source in expanded.sources {
            let (logical_path, entry, filename) = if let Some(origin) = &source.archive_origin {
                (
                    archive::member_url(&origin.archive_path, &origin.entry_name, false)
                        .to_string_lossy()
                        .into_owned(),
                    origin.entry_name.clone(),
                    Path::new(&origin.entry_name)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                (
                    source.path.to_string_lossy().into_owned(),
                    String::new(),
                    source
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                )
            };
            // Use the same tag, decoder, and legacy text-encoding rules as the
            // desktop queue. The complete row is sent to Swift in one step.
            let metadata = AudioTrack::from_source(source.clone(), &registry);
            let name = source.subsong.map_or(filename.clone(), |index| {
                format!("{filename} #{}", index + 1)
            });
            tracks.push(json!({
            "kind": "device",
            "path": logical_path,
            "entry": entry,
            "fragment": source.subsong.map_or(String::new(), |index| index.to_string()),
            "name": name,
            "title": metadata.title,
            "artist": metadata.artist,
            "album": metadata.album,
            "duration": metadata.duration.map_or(0, |duration| duration.as_millis().min(i64::MAX as u128) as i64),
        }));
        }
    }
    Ok(json!(tracks))
}

fn artwork(path: &Path) -> Option<Vec<u8>> {
    let file = if archive::is_tree_location(path) {
        // Keep the registry's extraction workspace alive until the image has
        // been read. The queue itself continues to hold the logical URL.
        let registry = DecoderRegistry::new(DecoderSettings::default());
        let source = registry
            .expand_detailed(path.to_path_buf())
            .ok()?
            .sources
            .into_iter()
            .next()?;
        let bytes = cover_art::embedded_cover_bytes(&source.path)
            .or_else(|| cover_art::sibling_cover_bytes(&source.path))?;
        return (bytes.len() <= cover_art::MAX_COVER_BYTES as usize).then_some(bytes);
    } else {
        path.to_path_buf()
    };
    let bytes =
        cover_art::embedded_cover_bytes(&file).or_else(|| cover_art::sibling_cover_bytes(&file))?;
    (bytes.len() <= cover_art::MAX_COVER_BYTES as usize).then_some(bytes)
}

unsafe fn output_json(
    result: Result<Value, String>,
    error: *mut c_char,
    capacity: usize,
) -> *mut c_char {
    match result {
        Ok(value) => CString::new(value.to_string())
            .expect("JSON cannot contain NUL")
            .into_raw(),
        Err(message) => {
            unsafe { error_to_buffer(&message, error, capacity) };
            ptr::null_mut()
        }
    }
}

/// Caller owns the JSON C string and must release it with `kog_audio_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_browse(
    root: *const c_char,
    path: *const c_char,
    error: *mut c_char,
    error_capacity: usize,
) -> *mut c_char {
    let result =
        input_path(root).and_then(|root| input_path(path).and_then(|path| browse(&root, &path)));
    unsafe { output_json(result, error, error_capacity) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_expand(
    path: *const c_char,
    error: *mut c_char,
    error_capacity: usize,
) -> *mut c_char {
    unsafe {
        output_json(
            input_path(path).and_then(|path| expand(&path)),
            error,
            error_capacity,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}

/// Returns owned JPEG/PNG bytes, or null when no local artwork exists.
/// The caller must release them with `kog_audio_bytes_free` and the returned length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_artwork(path: *const c_char, length: *mut usize) -> *mut u8 {
    if length.is_null() {
        return ptr::null_mut();
    }
    unsafe { *length = 0 };
    let Some(bytes) = input_path(path).ok().and_then(|path| artwork(&path)) else {
        return ptr::null_mut();
    };
    let boxed = bytes.into_boxed_slice();
    unsafe { *length = boxed.len() };
    Box::into_raw(boxed) as *mut u8
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_bytes_free(bytes: *mut u8, length: usize) {
    if !bytes.is_null() {
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(bytes, length)) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in entries {
            archive.start_file(name, options).unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
    }

    #[test]
    fn browses_nested_archive_folders_through_logical_paths() {
        let directory = tempfile::tempdir().unwrap();
        let inner = directory.path().join("inner.zip");
        write_zip(&inner, &[("Sub/tune.mid", b"MThd")]);
        let inner_bytes = std::fs::read(&inner).unwrap();
        let outer = directory.path().join("collection.zip");
        write_zip(
            &outer,
            &[("Disc/song.wav", b"RIFF"), ("Disc/inner.zip", &inner_bytes)],
        );
        let root = browse(directory.path(), &outer).unwrap();
        let disc = root["directories"][0]["path"].as_str().unwrap();
        let disc_listing = browse(directory.path(), Path::new(disc)).unwrap();
        assert_eq!(disc_listing["files"][0]["name"], "song.wav");
        let nested = disc_listing["directories"][0]["path"].as_str().unwrap();
        let nested_listing = browse(directory.path(), Path::new(nested)).unwrap();
        assert_eq!(nested_listing["directories"][0]["name"], "Sub");
        let sub = nested_listing["directories"][0]["path"].as_str().unwrap();
        let sub_listing = browse(directory.path(), Path::new(sub)).unwrap();
        assert_eq!(sub_listing["files"][0]["name"], "tune.mid");
    }

    #[test]
    fn browses_sandbox_folder_without_leaking_hidden_files() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("song.wav"), b"RIFF").unwrap();
        std::fs::write(directory.path().join("._song.wav"), b"ignored").unwrap();
        std::fs::create_dir(directory.path().join("Album")).unwrap();
        let listing = browse(directory.path(), directory.path()).unwrap();
        assert_eq!(listing["files"].as_array().unwrap().len(), 1);
        assert_eq!(listing["directories"].as_array().unwrap().len(), 1);
        assert_eq!(listing["files"][0]["name"], "song.wav");
    }

    #[test]
    fn returns_sibling_artwork_through_owned_c_buffer() {
        let directory = tempfile::tempdir().unwrap();
        let song = directory.path().join("song.wav");
        std::fs::write(&song, b"RIFF").unwrap();
        let art = b"\x89PNG\r\n\x1a\ncover";
        std::fs::write(directory.path().join("cover.png"), art).unwrap();
        let path = CString::new(song.to_str().unwrap()).unwrap();
        let mut length = 0;
        let pointer = unsafe { kog_audio_artwork(path.as_ptr(), &mut length) };
        assert!(!pointer.is_null());
        assert_eq!(unsafe { std::slice::from_raw_parts(pointer, length) }, art);
        unsafe { kog_audio_bytes_free(pointer, length) };
    }
}
