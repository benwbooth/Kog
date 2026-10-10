//! Offline copies of tracks streamed from a Kog server. Downloading saves the
//! original file (or the whole archive a track lives in, so companion files
//! come along) and remembers it; playing the remote track then plays the
//! local copy, so playlists keep working without the server.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use url::Url;

const INDEX_SETTING: &str = "offline-copies";

/// A downloaded track: the local file, and for an archive the member to play.
#[derive(Clone, Debug, PartialEq)]
pub struct OfflineCopy {
    pub path: PathBuf,
    pub entry: Option<String>,
    pub fragment: Option<String>,
}

/// What identifies a remote track regardless of codec or credentials: the
/// server, and the track's kind, path, archive entry and subsong.
pub fn key(stream_url: &str) -> Option<String> {
    let url = Url::parse(stream_url).ok()?;
    if !url.path().ends_with("/api/stream") {
        return None;
    }
    let pairs: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    let host = format!("{}://{}{}", url.scheme(), url.host_str()?, url.port().map(|p| format!(":{p}")).unwrap_or_default());
    Some(format!(
        "{host}\0{}\0{}\0{}\0{}",
        pairs.get("kind")?,
        pairs.get("path")?,
        pairs.get("entry").map(String::as_str).unwrap_or_default(),
        pairs.get("fragment").map(String::as_str).unwrap_or_default(),
    ))
}

fn load_index() -> BTreeMap<String, OfflineCopy> {
    let Some(text) = crate::settings::load_text(INDEX_SETTING) else { return BTreeMap::new() };
    let Ok(Value::Object(items)) = serde_json::from_str::<Value>(&text) else { return BTreeMap::new() };
    items
        .into_iter()
        .filter_map(|(key, value)| {
            Some((
                key,
                OfflineCopy {
                    path: PathBuf::from(value.get("path")?.as_str()?),
                    entry: value.get("entry").and_then(Value::as_str).map(str::to_owned),
                    fragment: value.get("fragment").and_then(Value::as_str).map(str::to_owned),
                },
            ))
        })
        .collect()
}

fn save_index(index: &BTreeMap<String, OfflineCopy>) -> Result<(), String> {
    let object: serde_json::Map<String, Value> = index
        .iter()
        .map(|(key, copy)| (key.clone(), json!({"path": copy.path, "entry": copy.entry, "fragment": copy.fragment})))
        .collect();
    crate::settings::save_text(INDEX_SETTING, &Value::Object(object).to_string())
}

/// The offline copy of a remote track, if one was downloaded and still exists.
pub fn find(stream_url: &str) -> Option<OfflineCopy> {
    let copy = load_index().remove(&key(stream_url)?)?;
    copy.path.exists().then_some(copy)
}

/// Forget a track's offline copy and delete the file when no other track
/// uses it (several songs can share one downloaded archive).
pub fn remove(stream_url: &str) -> Result<(), String> {
    let mut index = load_index();
    let Some(copy) = key(stream_url).and_then(|key| index.remove(&key)) else { return Ok(()) };
    if !index.values().any(|other| other.path == copy.path) {
        let _ = std::fs::remove_file(&copy.path);
    }
    save_index(&index)
}

/// The folder offline copies go to unless another is chosen.
pub fn default_folder() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|dirs| dirs.audio_dir().map(Path::to_path_buf))
        .or_else(|| directories::UserDirs::new().map(|dirs| dirs.home_dir().join("Music")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Kog offline")
}

/// Download a remote track into `folder` and remember it. A track inside an
/// archive downloads the whole archive, so the songs and files beside it play
/// too; an archive already downloaded for another track is reused.
pub fn download(stream_url: &str, folder: &Path) -> Result<OfflineCopy, String> {
    let key = key(stream_url).ok_or("This is not a track from a Kog server")?;
    if let Some(copy) = find(stream_url) {
        return Ok(copy);
    }
    let url = Url::parse(stream_url).map_err(|error| error.to_string())?;
    let pairs: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    let kind = pairs.get("kind").cloned().unwrap_or_default();
    let remote_path = pairs.get("path").cloned().ok_or("The track has no path")?;
    let entry = pairs.get("entry").cloned().filter(|entry| !entry.is_empty());
    let fragment = pairs.get("fragment").cloned();
    let (download_kind, entry) = match kind.as_str() {
        "local" => ("local", None),
        "archive" => ("local", entry),
        other => return Err(format!("Tracks of kind {other} cannot be downloaded")),
    };
    // An archive already saved for a sibling track.
    let mut index = load_index();
    let sibling = index.iter().find_map(|(other, copy)| {
        let mut parts = other.split('\0');
        let same = parts.next() == key.split('\0').next() && parts.next() == Some("archive") && parts.next() == Some(remote_path.as_str());
        (same && copy.path.exists()).then(|| copy.path.clone())
    });
    let path = match sibling {
        Some(path) => path,
        None => {
            let mut download = url.clone();
            download.set_path(&url.path().replace("/api/stream", "/api/media/download"));
            download.set_query(None);
            {
                let mut query = download.query_pairs_mut();
                query.append_pair("kind", download_kind).append_pair("path", &remote_path);
                if let Some(token) = pairs.get("token") {
                    query.append_pair("token", token);
                }
            }
            let authorization = (!url.username().is_empty()).then(|| {
                use base64::Engine;
                let user = percent_encoding::percent_decode_str(url.username()).decode_utf8_lossy();
                let password = url.password().map(|p| percent_encoding::percent_decode_str(p).decode_utf8_lossy().into_owned()).unwrap_or_default();
                format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}")))
            });
            let _ = download.set_username("");
            let _ = download.set_password(None);
            let name = Path::new(&remote_path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "track".into());
            std::fs::create_dir_all(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
            let path = crate::export::unused_path(folder, &name);
            fetch(download.as_str(), authorization, &path)?;
            path
        }
    };
    let copy = OfflineCopy { path, entry, fragment };
    index.insert(key, copy.clone());
    save_index(&index)?;
    Ok(copy)
}

fn fetch(url: &str, authorization: Option<String>, path: &Path) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_connect(Some(Duration::from_secs(15))).build().into();
    let mut request = agent.get(url);
    if let Some(value) = authorization {
        request = request.header("Authorization", value);
    }
    let mut response = request.call().map_err(|error| format!("Downloading failed: {error}"))?;
    let partial = path.with_extension("part");
    let result = (|| {
        let mut file = std::fs::File::create(&partial).map_err(|error| format!("{}: {error}", partial.display()))?;
        let mut body = response.body_mut().with_config().limit(u64::MAX).reader();
        std::io::copy(&mut body, &mut file).map_err(|error| format!("Downloading failed: {error}"))?;
        file.flush().map_err(|error| error.to_string())?;
        std::fs::rename(&partial, path).map_err(|error| format!("{}: {error}", path.display()))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    result
}

/// The local source to play instead of a remote one, when an offline copy
/// exists.
pub fn local_source(
    source: &crate::decoder::PlaybackSource,
    decoders: &crate::decoder::DecoderRegistry,
) -> Option<crate::decoder::PlaybackSource> {
    let copy = find(source.remote_url.as_deref()?)?;
    let stored = kog_core::db::StoredEntry {
        kind: if copy.entry.is_some() { "archive" } else { "local" }.to_owned(),
        path: copy.path.to_string_lossy().into_owned(),
        entry: copy.entry.clone().unwrap_or_default(),
        fragment: copy.fragment.clone(),
    };
    decoders.expand_queue_entry(&stored).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_ignore_codec_and_credentials() {
        let a = key("http://kog.local:7400/api/stream?kind=archive&path=%2Fm%2Fp.zip&entry=a.nsf&fragment=2&codec=aac&token=x").unwrap();
        let b = key("http://kog.local:7400/api/stream?token=y&codec=flac&kind=archive&path=%2Fm%2Fp.zip&entry=a.nsf&fragment=2").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, key("http://kog.local:7400/api/stream?kind=archive&path=%2Fm%2Fp.zip&entry=a.nsf&fragment=3").unwrap());
        assert!(key("https://example.com/song.mp3").is_none());
    }
}
