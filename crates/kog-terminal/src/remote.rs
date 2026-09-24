//! Small client for browsing another Kog server from the terminal frontend.
//! Requests run on the TUI's remote worker, never on its input thread.

use std::collections::HashSet;
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

#[derive(Clone, Debug, Deserialize)]
pub struct RemoteFile {
    pub name: String,
    pub path: String,
    #[serde(default = "local_kind")]
    pub kind: String,
    #[serde(default)]
    pub entry: String,
    pub fragment: Option<String>,
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
        let authorization = match self.auth_mode.as_str() {
            "token" if !self.token.is_empty() => Some(format!("Bearer {}", self.token)),
            "basic" if !self.username.is_empty() => Some(format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD
                    .encode(format!("{}:{}", self.username, self.password))
            )),
            _ => None,
        };
        if let Some(value) = authorization.as_deref() {
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

    pub fn search(
        &self,
        query: &str,
        cancelled: impl Fn() -> bool,
    ) -> Result<Vec<RemoteFile>, String> {
        let mut url = self.endpoint("/api/library/search")?;
        url.query_pairs_mut().append_pair("q", query);
        let mut page: serde_json::Value = self.get_json(url)?;
        let generation = page["generation"]
            .as_u64()
            .ok_or_else(|| "Server search response has no generation".to_owned())?;
        let mut results = Vec::new();
        loop {
            if cancelled() {
                return Err("Search superseded".to_owned());
            }
            for item in page["results"].as_array().into_iter().flatten() {
                let file: RemoteFile = serde_json::from_value(item.clone())
                    .map_err(|error| format!("Invalid server search result: {error}"))?;
                results.push(file);
            }
            if page["done"].as_bool().unwrap_or(false) || results.len() >= 2_000 {
                return Ok(results);
            }
            std::thread::sleep(Duration::from_millis(120));
            let mut url = self.endpoint("/api/library/search/more")?;
            url.query_pairs_mut()
                .append_pair("g", &generation.to_string())
                .append_pair("offset", &results.len().to_string());
            page = self.get_json(url)?;
        }
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

    pub fn collect_folder(&self, start: &Path) -> Result<Vec<RemoteFile>, String> {
        let mut pending = vec![start.to_string_lossy().into_owned()];
        let mut visited = HashSet::new();
        let mut files = Vec::new();
        while let Some(path) = pending.pop() {
            if !visited.insert(path.clone()) {
                continue;
            }
            let listing = self.browse(Some(&path))?;
            for directory in listing.directories.into_iter().rev() {
                pending.push(directory.path);
            }
            files.extend(listing.files);
            if files.len() > 50_000 || visited.len() > 50_000 {
                return Err("Remote folder contains too many entries".to_owned());
            }
        }
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
