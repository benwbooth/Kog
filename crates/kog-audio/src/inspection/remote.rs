//! Bounded, asynchronous metadata windows for a Kog HTTP audio stream.
use super::{Description, Snapshot, Window};
use base64::Engine;
use serde::Deserialize;
use std::collections::VecDeque;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct State {
    windows: VecDeque<Window>,
    pending: bool,
    requested: Option<Instant>,
    detail: String,
}

pub(crate) struct RemoteMonitor {
    endpoint: url::Url,
    authorization: Option<String>,
    offset: f64,
    shared: Arc<Mutex<State>>,
}

#[derive(Deserialize)]
struct Reply {
    window: Option<Window>,
    detail: Option<String>,
}

impl RemoteMonitor {
    pub fn new(location: &str) -> Option<Self> {
        let mut endpoint = url::Url::parse(location).ok()?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || !endpoint.path().ends_with("/api/stream")
        {
            return None;
        }
        let offset = endpoint
            .query_pairs()
            .find(|(key, _)| key == "start_ms")
            .and_then(|(_, value)| value.parse::<f64>().ok())
            .filter(|n| n.is_finite() && *n >= 0.0)
            .unwrap_or(0.0)
            / 1000.0;
        let path = endpoint.path().strip_suffix("stream")?.to_owned() + "inspection";
        endpoint.set_path(&path);
        let authorization = if endpoint.username().is_empty() {
            None
        } else {
            let user =
                percent_encoding::percent_decode_str(endpoint.username()).decode_utf8_lossy();
            let password =
                percent_encoding::percent_decode_str(endpoint.password().unwrap_or_default())
                    .decode_utf8_lossy();
            Some(format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
            ))
        };
        let _ = endpoint.set_username("");
        let _ = endpoint.set_password(None);
        Some(Self {
            endpoint,
            authorization,
            offset,
            shared: Arc::new(Mutex::new(State {
                windows: VecDeque::new(),
                pending: false,
                requested: None,
                detail: "Waiting for channel data from the streaming decoder…".into(),
            })),
        })
    }

    pub fn snapshot(&self, position: Duration, playing: bool, seeking: bool) -> Snapshot {
        let position = position.as_secs_f64() + self.offset;
        let mut state = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let window = state
            .windows
            .iter()
            .find(|w| position >= w.start && position < w.end);
        let snapshot = window
            .map(|w| w.snapshot(position, playing, seeking))
            .unwrap_or_else(|| Snapshot {
                version: 1,
                position,
                playing,
                seeking,
                description: Description {
                    backend: "Remote decoder".into(),
                    kind: "pending".into(),
                    detail: state.detail.clone(),
                },
                ..Snapshot::default()
            });
        let request = match window {
            None => Some(position),
            Some(window)
                if position > window.end - 0.3
                    && !state.windows.iter().any(|w| w.start >= window.end) =>
            {
                Some(window.end + 0.001)
            }
            _ => None,
        };
        if let Some(position) = request.filter(|_| {
            !state.pending
                && !seeking
                && state
                    .requested
                    .is_none_or(|when| when.elapsed() > Duration::from_millis(400))
        }) {
            state.pending = true;
            state.requested = Some(Instant::now());
            let mut endpoint = self.endpoint.clone();
            endpoint
                .query_pairs_mut()
                .append_pair("position", &position.to_string());
            let authorization = self.authorization.clone();
            let shared = self.shared.clone();
            let spawned = std::thread::Builder::new()
                .name("kog-channel-window".into())
                .spawn(move || {
                    let result = (|| -> Result<Reply, String> {
                        let agent: ureq::Agent = ureq::Agent::config_builder()
                            .timeout_global(Some(Duration::from_secs(8)))
                            .build()
                            .into();
                        let mut request = agent.get(endpoint.as_str());
                        if let Some(header) = authorization {
                            request = request.header("Authorization", header);
                        }
                        let response = request
                            .call()
                            .map_err(|_| "The server could not provide channel data.".to_owned())?;
                        serde_json::from_reader(
                            response.into_body().as_reader().take(64 * 1024 * 1024),
                        )
                        .map_err(|_| "The server returned invalid channel data.".into())
                    })();
                    let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
                    state.pending = false;
                    match result {
                        Ok(Reply {
                            window: Some(window),
                            ..
                        }) => {
                            state.windows.retain(|w| w.start != window.start);
                            state.windows.push_back(window);
                            while state.windows.len() > 3 {
                                state.windows.pop_front();
                            }
                        }
                        Ok(reply) => {
                            state.detail = reply
                                .detail
                                .unwrap_or_else(|| "Waiting for channel data…".into())
                        }
                        Err(error) => state.detail = error,
                    }
                });
            if spawned.is_err() {
                state.pending = false;
                state.detail = "Could not request channel data.".into();
            }
        }
        snapshot
    }
}
