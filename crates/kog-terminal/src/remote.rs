//! Small client for browsing another Kog server from the terminal frontend.
//! Requests run on the TUI's remote worker, never on its input thread.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteSettings {
    pub server_url: String,
    pub token: String,
    pub username: String,
    pub password: String,
    pub auth_mode: String,
    pub codec: String,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            token: String::new(),
            username: String::new(),
            password: String::new(),
            auth_mode: "token".to_owned(),
            codec: "aac".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct RemoteDirectory {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RemoteFile {
    pub name: String,
    pub path: String,
    #[serde(default = "local_kind")]
    pub kind: String,
    #[serde(default)]
    pub entry: String,
    pub fragment: Option<String>,
}

pub struct RemoteSearchHit {
    pub file: RemoteFile,
    pub is_dir: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RemoteSearchProgress {
    pub matches: usize,
    pub scanned: u64,
    pub archive_count: u64,
    pub archives_scanned: u64,
    pub unreadable_archives: u64,
    pub scanning_archives: bool,
}

fn local_kind() -> String {
    "local".to_owned()
}

#[derive(Clone, Debug, Deserialize)]
pub struct RemoteListing {
    pub path: String,
    #[serde(default)]
    pub directories: Vec<RemoteDirectory>,
    #[serde(default)]
    pub files: Vec<RemoteFile>,
}

impl RemoteSettings {
    pub fn load() -> Self {
        kog_audio::settings::setting_path("tui-remote-server.json")
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = kog_audio::settings::setting_path("tui-remote-server.json")
            .ok_or_else(|| "The Kog configuration directory is unavailable".to_owned())?;
        let parent = path
            .parent()
            .ok_or_else(|| "The Kog configuration directory is unavailable".to_owned())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("creating {}: {error}", parent.display()))?;
        let encoded = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if path.exists() {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                    .map_err(|error| format!("protecting {}: {error}", path.display()))?;
            }
        }
        let mut options = fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(&path)
            .and_then(|mut file| file.write_all(&encoded))
            .map_err(|error| format!("writing {}: {error}", path.display()))?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        let url = Url::parse(self.server_url.trim())
            .map_err(|_| "Enter a full http or https server URL".to_owned())?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("Enter a full http or https server URL".to_owned());
        }
        if !matches!(self.auth_mode.as_str(), "token" | "basic" | "none") {
            return Err("Choose Token, Password, or None authentication".to_owned());
        }
        if !matches!(self.codec.as_str(), "aac" | "opus" | "flac") {
            return Err("Choose AAC, Opus, or FLAC streaming".to_owned());
        }
        Ok(())
    }

    fn endpoint(&self, endpoint: &str) -> Result<Url, String> {
        self.validate()?;
        Url::parse(&format!(
            "{}{}",
            self.server_url.trim_end_matches('/'),
            endpoint
        ))
        .map_err(|error| format!("Invalid server address: {error}"))
    }

    pub fn browse(&self, path: Option<&str>) -> Result<RemoteListing, String> {
        let mut url = self.endpoint("/api/library")?;
        if let Some(path) = path {
            url.query_pairs_mut().append_pair("path", path);
        }
        self.get_json(url)
    }

    fn get_json<T: DeserializeOwned>(&self, url: Url) -> Result<T, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .into();
        let mut request = agent.get(url.as_str());
        if let Some(value) = self.authorization() {
            request = request.header("Authorization", value);
        }
        let response = request.call().map_err(|error| match error {
            ureq::Error::StatusCode(401) => {
                "Authentication failed; check the server token or password".to_owned()
            }
            _ => format!("Browsing {}: {error}", self.server_url),
        })?;
        serde_json::from_reader(response.into_body().as_reader())
            .map_err(|error| format!("Unexpected response from server: {error}"))
    }

    fn authorization(&self) -> Option<String> {
        match self.auth_mode.as_str() {
            "token" if !self.token.is_empty() => Some(format!("Bearer {}", self.token)),
            "basic" if !self.username.is_empty() => Some(format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD
                    .encode(format!("{}:{}", self.username, self.password))
            )),
            _ => None,
        }
    }

    fn search_control(&self, action: &str, generation: u64, paused: bool) -> Result<(), String> {
        let url = self.endpoint(&format!("/api/library/search/{action}"))?;
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .into();
        let mut request = agent.post(url.as_str());
        if let Some(value) = self.authorization() {
            request = request.header("Authorization", value);
        }
        request
            .send_json(serde_json::json!({ "generation": generation, "paused": paused }))
            .map_err(|error| format!("Could not {action} search: {error}"))?;
        Ok(())
    }

    /// Ask the server to turn multi-song locators into distinct playable
    /// entries. The HTTP route uses the same expansion rules as Qt and Web.
    pub fn expand_files(&self, files: &[RemoteFile]) -> Result<Vec<RemoteFile>, String> {
        #[derive(Deserialize)]
        struct Expanded {
            tracks: Vec<Vec<RemoteFile>>,
        }
        let mut result = Vec::new();
        for chunk in files.chunks(200) {
            let url = self.endpoint("/api/expand")?;
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(30)))
                .build()
                .into();
            let mut request = agent.post(url.as_str());
            match self.auth_mode.as_str() {
                "token" if !self.token.is_empty() => {
                    request = request.header("Authorization", format!("Bearer {}", self.token));
                }
                "basic" if !self.username.is_empty() => {
                    request = request.header(
                        "Authorization",
                        format!(
                            "Basic {}",
                            base64::engine::general_purpose::STANDARD
                                .encode(format!("{}:{}", self.username, self.password))
                        ),
                    );
                }
                _ => {}
            }
            let response = match request.send_json(chunk) {
                Ok(response) => response,
                Err(ureq::Error::StatusCode(404)) => {
                    // Older Kog servers predate /api/expand. Ordinary tracks
                    // still play; only multi-song splitting is unavailable.
                    result.extend_from_slice(chunk);
                    continue;
                }
                Err(error) => return Err(format!("Expanding remote tracks: {error}")),
            };
            let expanded: Expanded = serde_json::from_reader(response.into_body().as_reader())
                .map_err(|error| format!("Unexpected expansion response: {error}"))?;
            if expanded.tracks.len() != chunk.len() {
                return Err("Server returned an incomplete track expansion".to_owned());
            }
            for (original, tracks) in chunk.iter().zip(expanded.tracks) {
                if tracks.is_empty() {
                    result.push(original.clone());
                } else {
                    result.extend(tracks);
                }
            }
        }
        Ok(result)
    }

    pub fn search(
        &self,
        query: &str,
        cancelled: impl Fn() -> bool,
        paused: impl Fn() -> bool,
        mut progress: impl FnMut(&str, Vec<RemoteSearchHit>, RemoteSearchProgress),
    ) -> Result<(), String> {
        let root = self.browse(None)?.path;
        let mut url = self.endpoint("/api/library/search")?;
        url.query_pairs_mut().append_pair("q", query);
        let mut page: serde_json::Value = self.get_json(url)?;
        let generation = page["generation"]
            .as_u64()
            .ok_or_else(|| "Server search response has no generation".to_owned())?;
        let mut offset = 0;
        let mut server_paused = false;
        let result = (|| {
            loop {
                if cancelled() {
                    return Err("Search superseded".to_owned());
                }
                let requested_pause = paused();
                if server_paused != requested_pause {
                    self.search_control("pause", generation, requested_pause)?;
                    server_paused = requested_pause;
                }
                let mut results = Vec::new();
                for item in page["results"].as_array().into_iter().flatten() {
                    let file: RemoteFile = serde_json::from_value(item.clone())
                        .map_err(|error| format!("Invalid server search result: {error}"))?;
                    results.push(RemoteSearchHit {
                        file,
                        is_dir: item["is_dir"].as_bool().unwrap_or(false),
                    });
                }
                offset += results.len();
                progress(
                    &root,
                    results,
                    RemoteSearchProgress {
                        matches: page["total"].as_u64().unwrap_or(offset as u64) as usize,
                        scanned: page["scanned"].as_u64().unwrap_or_default(),
                        archive_count: page["archive_count"].as_u64().unwrap_or_default(),
                        archives_scanned: page["archives_scanned"].as_u64().unwrap_or_default(),
                        unreadable_archives: page["unreadable_archives"]
                            .as_u64()
                            .unwrap_or_default(),
                        scanning_archives: page["scanning_archives"].as_bool().unwrap_or(false),
                    },
                );
                let total = page["total"].as_u64().unwrap_or(offset as u64) as usize;
                if (page["done"].as_bool().unwrap_or(false) && offset >= total) || offset >= 2_000 {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(120));
                let mut url = self.endpoint("/api/library/search/more")?;
                url.query_pairs_mut()
                    .append_pair("g", &generation.to_string())
                    .append_pair("offset", &offset.to_string());
                page = self.get_json(url)?;
            }
        })();
        if result.is_err() {
            let _ = self.search_control("cancel", generation, false);
        }
        result
    }

    pub fn stream_url(&self, file: &RemoteFile) -> Result<String, String> {
        let mut url = self.endpoint("/api/stream")?;
        if self.auth_mode == "basic" && !self.username.is_empty() {
            url.set_username(&self.username)
                .map_err(|_| "Invalid server username".to_owned())?;
            url.set_password(Some(&self.password))
                .map_err(|_| "Invalid server password".to_owned())?;
        }
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("kind", &file.kind)
                .append_pair("path", &file.path)
                .append_pair("codec", &self.codec);
            if !file.entry.is_empty() {
                query.append_pair("entry", &file.entry);
            }
            if let Some(fragment) = file.fragment.as_deref() {
                query.append_pair("fragment", fragment);
            }
            if self.auth_mode == "token" && !self.token.is_empty() {
                query.append_pair("token", &self.token);
            }
        }
        Ok(url.into())
    }

    pub fn collect_folder(&self, start: &Path, query: &str) -> Result<Vec<RemoteFile>, String> {
        #[derive(Deserialize)]
        struct Collected { tracks: Vec<RemoteFile> }
        let mut url = self.endpoint("/api/library/collect")?;
        url.query_pairs_mut().append_pair("path", &start.to_string_lossy()).append_pair("q", query);
        let result: Collected = self.get_json(url)?;
        Ok(result.tracks)
    }
}

impl RemoteFile {
    pub fn from_stream_url(name: String, stream_url: &str) -> Option<Self> {
        let url = Url::parse(stream_url).ok()?;
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        Some(Self {
            name,
            path: pairs.get("path")?.clone(),
            kind: pairs.get("kind")?.clone(),
            entry: pairs.get("entry").cloned().unwrap_or_default(),
            fragment: pairs.get("fragment").cloned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search_server(
        exchanges: Vec<(&'static str, Option<serde_json::Value>, serde_json::Value)>,
    ) -> (RemoteSettings, std::thread::JoinHandle<()>) {
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let settings = RemoteSettings {
            server_url: format!("http://{}", listener.local_addr().unwrap()),
            token: "test-search-token".into(),
            ..RemoteSettings::default()
        };
        let worker = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            for (expected, body, response) in exchanges {
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "missing request: {expected}"
                            );
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("{error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                    if bytes.ends_with(b"\r\n\r\n") {
                        break;
                    }
                }
                let headers = String::from_utf8(bytes).unwrap();
                assert!(
                    headers.starts_with(expected),
                    "expected {expected}, got {headers}"
                );
                assert!(
                    headers
                        .to_lowercase()
                        .contains("authorization: bearer test-search-token")
                );
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                let mut actual_body = vec![0; length];
                stream.read_exact(&mut actual_body).unwrap();
                if let Some(body) = body {
                    assert_eq!(
                        serde_json::from_slice::<serde_json::Value>(&actual_body).unwrap(),
                        body
                    );
                }
                let response = response.to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            }
        });
        (settings, worker)
    }

    #[test]
    fn remote_search_streams_hits_and_pauses_resumes_and_cancels_its_generation() {
        use serde_json::json;
        let page = json!({"generation":7,"results":[],"total":1,"done":false,"scanned":12});
        let mut first = page.clone();
        first["results"] = json!([{"name":"Phantasy Star.mp3","path":"/music/Phantasy Star.mp3"}]);
        let (settings, server) = search_server(vec![
            ("GET /api/library HTTP/", None, json!({"path":"/music"})),
            ("GET /api/library/search?q=Phantasy+Star HTTP/", None, first),
            (
                "GET /api/library/search/more?g=7&offset=1 HTTP/",
                None,
                page.clone(),
            ),
            (
                "POST /api/library/search/pause HTTP/",
                Some(json!({"generation":7,"paused":true})),
                json!({"ok":true}),
            ),
            (
                "GET /api/library/search/more?g=7&offset=1 HTTP/",
                None,
                page.clone(),
            ),
            (
                "POST /api/library/search/pause HTTP/",
                Some(json!({"generation":7,"paused":false})),
                json!({"ok":true}),
            ),
            (
                "GET /api/library/search/more?g=7&offset=1 HTTP/",
                None,
                page,
            ),
            (
                "POST /api/library/search/cancel HTTP/",
                Some(json!({"generation":7,"paused":false})),
                json!({"ok":true}),
            ),
        ]);
        let paused = std::cell::Cell::new(false);
        let cancelled = std::cell::Cell::new(false);
        let mut batches = 0;
        let result = settings.search(
            "Phantasy Star",
            || cancelled.get(),
            || paused.get(),
            |root, hits, progress| {
                assert_eq!(root, "/music");
                assert_eq!(progress.scanned, 12);
                batches += 1;
                if batches == 1 {
                    assert_eq!(hits.len(), 1, "hits must be delivered before completion");
                    paused.set(true);
                } else {
                    assert!(hits.is_empty(), "polling must not duplicate results");
                    paused.set(false);
                    if batches == 3 {
                        cancelled.set(true);
                    }
                }
            },
        );
        assert_eq!(result.unwrap_err(), "Search superseded");
        server.join().unwrap();
    }

    #[test]
    fn remote_search_drains_all_pages_even_when_scanner_is_done() {
        use serde_json::json;
        let page = |name| json!({"generation":7,"results":[{"name":name,"path":format!("/music/{name}")}],"total":2,"done":true});
        let (settings, server) = search_server(vec![
            ("GET /api/library HTTP/", None, json!({"path":"/music"})),
            (
                "GET /api/library/search?q=Star HTTP/",
                None,
                page("Star 1.mp3"),
            ),
            (
                "GET /api/library/search/more?g=7&offset=1 HTTP/",
                None,
                page("Star 2.mp3"),
            ),
        ]);
        let mut hits = Vec::new();
        settings
            .search("Star", || false, || false, |_, batch, _| hits.extend(batch))
            .unwrap();
        assert_eq!(hits.len(), 2);
        server.join().unwrap();
    }

    #[test]
    fn stream_url_preserves_archive_locator_and_token() {
        let settings = RemoteSettings {
            server_url: "https://example.test/kog/".to_owned(),
            token: "a b/c".to_owned(),
            ..RemoteSettings::default()
        };
        let file = RemoteFile {
            name: "song.mid".to_owned(),
            path: "/music/sonic pack.zip".to_owned(),
            kind: "archive".to_owned(),
            entry: "inner/song.mid".to_owned(),
            fragment: Some("track 2".to_owned()),
        };
        let url = Url::parse(&settings.stream_url(&file).unwrap()).unwrap();
        assert_eq!(url.path(), "/kog/api/stream");
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(pairs["kind"], "archive");
        assert_eq!(pairs["path"], "/music/sonic pack.zip");
        assert_eq!(pairs["entry"], "inner/song.mid");
        assert_eq!(pairs["fragment"], "track 2");
        assert_eq!(pairs["token"], "a b/c");
    }

    #[test]
    fn stream_url_round_trips_source_for_expansion() {
        let settings = RemoteSettings {
            server_url: "https://example.test".to_owned(),
            token: "a b".to_owned(),
            ..RemoteSettings::default()
        };
        let file = RemoteFile {
            name: "game.nsf".to_owned(),
            path: "/music/game.nsf".to_owned(),
            kind: "local".to_owned(),
            entry: String::new(),
            fragment: None,
        };
        let url = settings.stream_url(&file).unwrap();
        let recovered = RemoteFile::from_stream_url(file.name.clone(), &url).unwrap();
        assert_eq!(recovered.path, file.path);
        assert_eq!(recovered.kind, file.kind);
        assert_eq!(recovered.name, file.name);
    }
}
