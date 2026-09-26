//! HTTP surface: health/version, the authenticated API root, and (soon) the
//! streaming endpoints. Handlers stay thin; the interesting logic lives in
//! `config` and `auth` so it can be tested without a socket.

use std::sync::Arc;

use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use tokio::sync::RwLock;

use kog_audio::decoder::DecoderSettings;

use crate::auth::{AuthError, ConfigView, authorize};
use crate::stream::StreamKey;
use crate::{ServerConfig, StreamCodec};

/// Shared server state. The config is behind a lock so the Preferences pane can
/// change it while the server runs.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<ServerConfig>>,
    pub version: &'static str,
    pub streams: Arc<crate::service::StreamService>,
    /// Library browsing and the playlist/star store.
    pub library: Arc<crate::api::Library>,
    /// The single resumable library-search walk.
    pub search: Arc<crate::api::SearchState>,
    /// Server-owned random radio, sharing the desktop's round file.
    pub radio: Arc<crate::radio::Radio>,
}

impl AppState {
    pub fn new(
        config: ServerConfig,
        version: &'static str,
        streams: crate::service::StreamService,
        library: crate::api::Library,
    ) -> Self {
        Self::with_radio(
            config,
            version,
            streams,
            library,
            crate::radio::Radio::disabled(),
        )
    }

    /// Build state with an explicit radio session. The desktop server uses
    /// this so radio reads and writes Kog's real settings and round file;
    /// tests get the disabled default from [`AppState::new`].
    pub fn with_radio(
        config: ServerConfig,
        version: &'static str,
        streams: crate::service::StreamService,
        library: crate::api::Library,
        radio: crate::radio::Radio,
    ) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            version,
            streams: Arc::new(streams),
            library: Arc::new(library),
            search: Arc::new(crate::api::SearchState::default()),
            radio: Arc::new(radio),
        }
    }

    /// Build the streaming service from the running configuration.
    pub fn stream_service(config: &ServerConfig, decoder_settings: DecoderSettings) -> crate::service::StreamService {
        crate::service::StreamService::new(
            crate::stream::StreamCache::new(
                crate::service::cache_root(),
                config.cache_bytes,
            ),
            decoder_settings,
            crate::service::scratch_root(),
        )
    }
}

/// Build the router. Kept separate from `serve` so tests can drive it
/// in-process with `tower::ServiceExt::oneshot`.
pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/codecs", get(codecs))
        .route("/api/config", get(read_config))
        .route("/api/devices", get(list_devices))
        .route("/api/devices/block", post(set_device_blocked))
        .route("/api/stream", get(stream_audio))
        .merge(crate::api::router())
        .merge(crate::radio::router())
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/api/health", get(health))
        .route("/api/version", get(version))
        .merge(protected)
        // Anything else is the web frontend, so a phone can bookmark the
        // server's own address and get the player.
        .fallback(web_asset)
        .with_state(state)
        .layer(middleware::from_fn(log_request))
}

/// Log each request with the client's user agent. The frontend runs in the
/// browser, where a blank page leaves no trace on the server; without this a
/// client that never fetches the wasm is impossible to tell from one that
/// fetches it and fails to start.
async fn log_request(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let agent = request
        .headers()
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("-")
        .to_owned();
    // The web player sends its id as a header; the audio element cannot, so
    // its stream URL carries the same id as a query parameter.
    let device_id = request
        .headers()
        .get("x-kog-device")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .or_else(|| {
            request.uri().query().and_then(|query| {
                query.split('&').find_map(|pair| {
                    pair.strip_prefix("device=").map(str::to_owned)
                })
            })
        })
        .unwrap_or_else(|| format!("agent:{agent}"));
    let addr = request
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|info| info.0.to_string())
        .unwrap_or_else(|| "-".to_owned());
    crate::devices::registry().record(device_id.clone(), agent.clone(), addr);
    // A device the user disconnected is refused at the API boundary: the web
    // frontend itself still loads, so it can say so instead of spinning.
    if path.starts_with("/api") && crate::devices::registry().blocked(&device_id) {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": "This device was disconnected in Kog's settings"
            })),
        )
            .into_response();
    }
    let response = next.run(request).await;
    // Streaming is chatty and long-lived; a page load is what matters here.
    if !path.starts_with("/api/stream") {
        eprintln!(
            "kog-server: {method} {path} -> {} | {agent}",
            response.status().as_u16()
        );
    }
    response
}

/// `GET /api/devices` — the clients seen at the API, most recent first.
async fn list_devices() -> impl IntoResponse {
    let devices = crate::devices::registry().list();
    axum::Json(serde_json::json!({ "devices": devices })).into_response()
}

/// `POST /api/devices/block` — cut a device off, or let it back in.
async fn set_device_blocked(
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let Some(id) = body["id"].as_str() else {
        return bad_request("a device id is required");
    };
    let blocked = body["blocked"].as_bool().unwrap_or(true);
    if crate::devices::registry().set_blocked(id, blocked) {
        axum::Json(serde_json::json!({ "ok": true, "id": id, "blocked": blocked }))
            .into_response()
    } else {
        bad_request(&format!("no device {id} has been seen"))
    }
}

async fn health() -> impl IntoResponse {
    axum::Json(serde_json::json!({ "ok": true }))
}

async fn version(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "name": "kog",
        "version": state.version,
        "api": 1,
    }))
}

/// Which codecs this server can transcode to, so the web client can offer a
/// choice without hardcoding it.
async fn codecs() -> impl IntoResponse {
    let codecs: Vec<serde_json::Value> = StreamCodec::ALL
        .iter()
        .map(|codec| {
            serde_json::json!({
                "id": codec.setting_value(),
                "contentType": codec.content_type(),
            })
        })
        .collect();
    axum::Json(serde_json::json!({ "codecs": codecs }))
}

/// The running configuration, minus anything secret.
async fn read_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(serde_json::json!({
        "enabled": config.enabled,
        "address": config.address.to_string(),
        "port": config.port,
        "auth": config.auth,
        "defaultCodec": config.default_codec.setting_value(),
        "tls": config.tls.mode,
    }))
}

/// Streaming request parameters. The locator fields mirror the library's
/// stored-entry shape (`kind`, `path`, `entry`, `fragment`), so stars,
/// playlists and streaming all address a track the same way.
#[derive(Debug, serde::Deserialize)]
pub struct StreamQuery {
    /// `local`, `archive` or `remote`.
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub entry: String,
    #[serde(default)]
    pub fragment: String,
    pub codec: Option<String>,
    pub bitrate: Option<u16>,
}

impl StreamQuery {
    /// The cache locator: the same key shape the library uses for identity, so
    /// one track maps to one cache entry regardless of which client asked.
    pub fn locator(&self) -> String {
        let base = match (self.kind.as_str(), self.entry.is_empty()) {
            ("archive", false) => format!("{}::{}", self.path, self.entry),
            _ => self.path.clone(),
        };
        if self.fragment.trim().is_empty() {
            base
        } else {
            format!("{base}#{}", self.fragment.trim())
        }
    }

    fn location(&self) -> Result<kog_audio::playlist::PlaylistLocation, String> {
        kog_audio::playlist::PlaylistEntry::from_locator(
            &self.kind,
            &self.path,
            &self.entry,
            (!self.fragment.trim().is_empty()).then(|| self.fragment.trim().to_owned()),
        )
        .map(|entry| entry.location)
    }

    fn codec(&self, default_codec: StreamCodec) -> Result<StreamCodec, String> {
        match self.codec.as_deref() {
            None | Some("") => Ok(default_codec),
            Some(value) => {
                StreamCodec::from_setting(value).ok_or_else(|| format!("unknown codec: {value}"))
            }
        }
    }
}

fn stream_key_for_request(
    query: &StreamQuery,
    codec: StreamCodec,
    bitrate: u16,
    midi_engine: kog_audio::settings::MidiEngine,
) -> StreamKey {
    let key = StreamKey::new(query.locator(), codec, bitrate);
    let filename = if query.kind == "archive" && !query.entry.is_empty() {
        &query.entry
    } else {
        &query.path
    };
    let uses_midi_engine = std::path::Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(kog_audio::decoder::uses_selected_midi_engine);
    if uses_midi_engine {
        key.with_render_profile(midi_engine.setting_value())
    } else {
        key
    }
}

/// Serve one track. Local MP3 files can go straight to the browser with byte
/// ranges; transcoding an hours-long MP3 would leave it unseekable until the
/// entire encode finished. Other sources use the cached or progressive encode.
async fn stream_audio(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<StreamQuery>,
    headers: axum::http::HeaderMap,
) -> Response {
    let default_codec = { state.config.read().await.default_codec };
    let codec = match query.codec(default_codec) {
        Ok(codec) => codec,
        Err(error) => return bad_request(&error),
    };
    let location = match query.location() {
        Ok(location) => location,
        Err(error) => return bad_request(&error),
    };
    let entry = kog_audio::playlist::PlaylistEntry {
        location,
        fragment: (!query.fragment.trim().is_empty()).then(|| query.fragment.trim().to_owned()),
    };
    if query.kind == "local"
        && query.entry.is_empty()
        && entry.fragment.is_none()
        && std::path::Path::new(&query.path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"))
    {
        return serve_file(std::path::Path::new(&query.path), "audio/mpeg", &headers).await;
    }
    let bitrate = query.bitrate.unwrap_or(crate::stream::DEFAULT_BITRATE_KBPS);
    let key = stream_key_for_request(&query, codec, bitrate, state.streams.midi_engine());

    // Cache lookup, decoder resolution and the encoder check all touch the
    // filesystem, so they run on a blocking thread; failures come back as a
    // real HTTP error instead of a 200 with an empty body.
    let streams = state.streams.clone();
    let opened = tokio::task::spawn_blocking(move || streams.open(entry, key))
        .await
        .unwrap_or_else(|error| Err(format!("stream setup failed: {error}")));

    match opened {
        Ok(crate::service::StreamSource::Cached(path)) => {
            serve_file(&path, codec.content_type(), &headers).await
        }
        Ok(crate::service::StreamSource::Encoding { receiver }) => {
            // Progressive: no Range support until the cache entry exists, but
            // playback starts immediately instead of waiting for the encode.
            let body = axum::body::Body::from_stream(
                tokio_stream::wrappers::ReceiverStream::new(receiver),
            );
            let mut response = Response::new(body);
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static(codec.content_type()));
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
        Err(error) => {
            eprintln!("kog-server: stream request failed: {error}");
            bad_request(&error)
        }
    }
}

/// Serve a local source or finished encode with byte ranges so browser seeking
/// works regardless of the file's duration.
async fn serve_file(
    path: &std::path::Path,
    content_type: &'static str,
    headers: &axum::http::HeaderMap,
) -> Response {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return bad_request("the stream file is unavailable");
    };
    let Ok(metadata) = file.metadata().await else {
        return bad_request("the stream file is unavailable");
    };
    if !metadata.is_file() {
        return bad_request("the stream file is unavailable");
    }
    let total = metadata.len();
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| parse_range(value, total));

    let (start, end) = match range {
        Some(range) => range,
        None => {
            let body = axum::body::Body::from_stream(
                tokio_util::io::ReaderStream::new(file),
            );
            let mut response = Response::new(body);
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
            response
                .headers_mut()
                .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response.headers_mut().insert(
                header::CONTENT_LENGTH,
                HeaderValue::from(total),
            );
            return response;
        }
    };

    let length = end - start + 1;
    if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return bad_request("the stream file could not be read");
    }
    let body = axum::body::Body::from_stream(
        tokio_util::io::ReaderStream::new(file.take(length)),
    );
    let mut response = Response::new(body);
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, HeaderValue::from(length));
    if let Ok(value) = HeaderValue::from_str(&format!("bytes {start}-{end}/{total}")) {
        response
            .headers_mut()
            .insert(header::CONTENT_RANGE, value);
    }
    *response.status_mut() = StatusCode::PARTIAL_CONTENT;
    response
}

/// Parse a single `bytes=start-end` range. Returns inclusive bounds clamped to
/// the file, or `None` when the request is not a satisfiable byte range.
pub fn parse_range(value: &str, total: u64) -> Option<(u64, u64)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    // Multi-range requests are answered with the whole file: browsers fall back
    // happily, and it keeps the response a single seekable stream.
    if spec.contains(',') {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", "") => return None,
        ("", suffix) => {
            let suffix: u64 = suffix.parse().ok()?;
            if suffix == 0 || total == 0 {
                return None;
            }
            (total.saturating_sub(suffix), total - 1)
        }
        (start, "") => (start.parse().ok()?, total.checked_sub(1)?),
        (start, end) => (start.parse().ok()?, end.parse::<u64>().ok()?.min(total.checked_sub(1)?)),
    };
    (start <= end && start < total).then_some((start, end))
}

/// The frontend built by `crates/kog-web/build.sh`, embedded so the server is
/// a single artifact and the UI can never version-skew from the API it talks
/// to. A placeholder page ships when the frontend has not been built.
static WEB_ASSETS: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/web");

async fn web_asset(uri: axum::http::Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    // "/" is the frontend; a build without one still explains itself instead
    // of returning a bare 404.
    let path = if path.is_empty() { "index.html" } else { path };
    let path = if WEB_ASSETS.get_file(path).is_some() {
        path
    } else if path == "index.html" {
        "placeholder.html"
    } else {
        path
    };
    match WEB_ASSETS.get_file(path) {
        Some(file) => {
            let content_type = content_type_for(path);
            // The assets sit at fixed URLs and a wasm-bindgen pair only works
            // when the script and the module come from the same build. Browsers
            // cache them heuristically when nothing says otherwise, so a rebuild
            // could pair an old script with a new wasm and render a blank page.
            // The page itself always revalidates; versioned asset URLs (they
            // carry this build's etag) are immutable and cache for good, and
            // unversioned extras ride a short window.
            let has_version = uri.query().is_some();
            let cache_control = match path {
                "index.html" | "manifest.webmanifest" => "no-cache",
                _ if has_version => "public, max-age=31536000, immutable",
                _ => "public, max-age=300",
            };
            // Precompressed variants: the wasm is the whole boot cost on a
            // phone. index.html and kog_web.js are served uncompressed because
            // their asset URLs get rewritten below.
            let accepts_gzip = headers
                .get(header::ACCEPT_ENCODING)
                .and_then(|value| value.to_str().ok())
                .map(|value| value.to_ascii_lowercase().contains("gzip"))
                .unwrap_or(false);
            let gz_path = format!("{path}.gz");
            let gz = accepts_gzip
                && !matches!(path, "index.html" | "kog_web.js")
                && WEB_ASSETS.get_file(&gz_path).is_some();
            // Version the cross-references with the build ETag: a browser (or
            // extension) that ignores no-cache still sees a brand-new URL per
            // build instead of serving stale bytes for weeks.
            let etag = build_etag();
            let version = format!(
                "?v={}",
                etag.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>()
            );
            let versioned = |body: String| {
                body.replace("kog_web_bg.wasm", &format!("kog_web_bg.wasm{version}"))
                    .replace(
                        "import(\"/kog_web.js\")",
                        &format!("import(\"/kog_web.js{version}\")"),
                    )
                    .replace("\"/style.css\"", &format!("\"/style.css{version}\""))
            };
            let text_body = match path {
                "index.html" | "kog_web.js" => Some(versioned(
                    String::from_utf8_lossy(file.contents()).into_owned(),
                )),
                _ => None,
            };
            let unchanged = headers
                .get(header::IF_NONE_MATCH)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.split(',').any(|part| part.trim() == etag));
            let mut response = if unchanged {
                let mut response = Response::new(axum::body::Body::empty());
                *response.status_mut() = StatusCode::NOT_MODIFIED;
                response
            } else {
                let body = match (text_body.as_deref(), gz) {
                    (_, true) => {
                        let compressed = WEB_ASSETS.get_file(&gz_path).expect("checked above");
                        axum::body::Body::from(compressed.contents().to_vec())
                    }
                    (Some(text), false) => axum::body::Body::from(text.as_bytes().to_vec()),
                    (None, false) => axum::body::Body::from(file.contents().to_vec()),
                };
                let mut response = Response::new(body);
                if gz {
                    response.headers_mut().insert(
                        header::CONTENT_ENCODING,
                        HeaderValue::from_static("gzip"),
                    );
                    response.headers_mut().insert(
                        header::VARY,
                        HeaderValue::from_static("Accept-Encoding"),
                    );
                }
                response
                    .headers_mut()
                    .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
                response
            };
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static(cache_control),
            );
            response.headers_mut().insert(
                header::ETAG,
                HeaderValue::from_str(&etag).expect("an etag is ASCII"),
            );
            response
        }
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({ "ok": false, "error": "not found" })),
        )
            .into_response(),
    }
}

/// One validator for the whole embedded frontend. Any asset changing (the
/// wasm, the script, or a stylesheet-only rebuild) changes every asset's ETag,
/// so a browser revalidates the complete build and the page's live-reload poll
/// notices a CSS-only change too. Hashing the bytes also stops a rebuild whose
/// length repeats from pairing a stale script with a new module.
fn build_etag() -> &'static str {
    use std::hash::{Hash, Hasher};

    fn walk(dir: &include_dir::Dir<'_>, hasher: &mut impl Hasher) {
        for file in dir.files() {
            file.path().hash(hasher);
            file.contents().hash(hasher);
        }
        for sub in dir.dirs() {
            walk(sub, hasher);
        }
    }

    static ETAG: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ETAG.get_or_init(|| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        walk(&WEB_ASSETS, &mut hasher);
        format!("W/\"build-{:x}\"", hasher.finish())
    })
}

fn content_type_for(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        Some("webmanifest") => "application/manifest+json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

pub(crate) fn bad_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        axum::Json(serde_json::json!({ "ok": false, "error": message })),
    )
        .into_response()
}

/// Reject unauthenticated requests before they reach a handler.
async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let (mode, token, credentials) = {
        let config = state.config.read().await;
        (
            config.auth,
            config.token.clone(),
            config.credentials.clone(),
        )
    };
    let view = ConfigView {
        token: &token,
        credentials: &credentials,
    };
    let header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    // The browser's audio element cannot set headers; it carries the token as
    // a query parameter on stream URLs instead.
    let query_token = if header.is_none() && mode == crate::AuthMode::Token {
        request
            .uri()
            .query()
            .and_then(|query| {
                url::form_urlencoded::parse(query.as_bytes())
                    .find(|(key, _)| key == "token")
                    .map(|(_, value)| format!("Bearer {value}"))
            })
    } else {
        None
    };
    let header = header.or_else(|| query_token.as_deref());
    match authorize(header, mode, &view) {
        Ok(()) => next.run(request).await,
        Err(error) => unauthorized(error),
    }
}

fn unauthorized(error: AuthError) -> Response {
    let body = axum::Json(serde_json::json!({
        "ok": false,
        "error": match error {
            AuthError::Missing => "authentication required",
            AuthError::Invalid => "invalid credentials",
        },
    }));
    let mut response = (StatusCode::UNAUTHORIZED, body).into_response();
    if let Ok(challenge) = HeaderValue::from_str(error.challenge()) {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, challenge);
    }
    response
}

/// Bind and serve forever, over TLS when configured.
pub async fn serve(state: AppState) -> Result<(), String> {
    serve_with_shutdown(state, std::future::pending::<()>()).await
}

/// Bind and serve until `shutdown` resolves. The desktop app uses this so the
/// Preferences pane can start and stop the API server without restarting Kog.
pub async fn serve_with_shutdown(
    state: AppState,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<(), String> {
    let config = {
        let config = state.config.read().await;
        config.clone()
    };
    if !config.enabled {
        return Err("the API server is disabled".to_owned());
    }
    // Refuse to start on a configuration that would be unsafe or unusable.
    config.validate()?;
    let address = config.socket_address();
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| format!("binding {address}: {error}"))?;
    let app = router(state);
    let scheme = if config.tls.mode == crate::TlsMode::Off { "http" } else { "https" };
    eprintln!("kog-server: listening on {scheme}://{address}");
    if config.tls.mode == crate::TlsMode::Off {
        return axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(|error| format!("serving on {address}: {error}"));
    }
    serve_tls(listener, app, &config, address, shutdown).await
}

/// TLS accept loop. Uses hyper's auto connection builder so HTTP/1.1 and
/// HTTP/2 clients both work over the encrypted socket.
async fn serve_tls(
    listener: tokio::net::TcpListener,
    app: Router,
    config: &ServerConfig,
    address: std::net::SocketAddr,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<(), String> {
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use hyper_util::server::conn::auto::Builder;

    let tls = crate::tls::server_config(&config.tls, address)?;
    let acceptor = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(tls));
    tokio::pin!(shutdown);
    loop {
        let accepted = tokio::select! {
            result = listener.accept() => result,
            () = &mut shutdown => return Ok(()),
        };
        let (stream, peer) = accepted
            .map_err(|error| format!("accepting on {address}: {error}"))?;
        let acceptor = acceptor.clone();
        let app = app.clone();
        tokio::spawn(async move {
            match acceptor.accept(stream).await {
                Ok(stream) => {
                    let service = hyper_util::service::TowerToHyperService::new(app);
                    let builder = Builder::new(TokioExecutor::new());
                    let connection =
                        builder.serve_connection_with_upgrades(TokioIo::new(stream), service);
                    if let Err(error) = connection.await {
                        eprintln!("kog-server: TLS connection from {peer} ended: {error}");
                    }
                }
                Err(error) => {
                    // A failed handshake is routine (port scanners, wrong
                    // scheme); report it without stopping the server.
                    eprintln!("kog-server: TLS handshake with {peer} failed: {error}");
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthMode;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    /// A service backed by a throwaway cache. The tempdir is leaked so it
    /// outlives the state the router holds.
    fn streams() -> crate::service::StreamService {
        let directory = Box::leak(Box::new(tempfile::tempdir().unwrap()));
        crate::service::StreamService::new(
            crate::stream::StreamCache::new(directory.path().join("streams"), 1 << 20),
            kog_audio::decoder::DecoderSettings::default(),
            directory.path().join("scratch"),
        )
    }

    /// A minimal, valid 8-bit mono WAV, so the decoder registry accepts it
    /// whether it filters by extension or by probing the content.
    fn write_wav(path: &std::path::Path) {
        let data: &[u8] = &[0x80];
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36_u32 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(data);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, wav).unwrap();
    }

    /// A WAV "id3 " chunk carrying an ID3v2.3 tag with TPE2 (album artist)
    /// and TCOM (composer) text frames, for fixtures that exercise the tag
    /// path. Sizes are big-endian; the header size is synchsafe as v2.3
    /// requires.
    fn id3_chunk(album_artist: &str, composer: &str) -> Vec<u8> {
        let frame = |id: &[u8; 4], text: &str| {
            let mut frame = Vec::new();
            frame.extend_from_slice(id);
            // Payload: encoding byte + text + terminating null.
            frame.extend_from_slice(&(text.len() as u32 + 2).to_be_bytes());
            frame.extend_from_slice(&[0, 0]);
            frame.push(0); // ISO-8859-1
            frame.extend_from_slice(text.as_bytes());
            frame.push(0);
            frame
        };
        let mut body = Vec::new();
        body.extend(frame(b"TPE2", album_artist));
        body.extend(frame(b"TCOM", composer));
        let synchsafe = |total: u32, out: &mut Vec<u8>| {
            out.extend_from_slice(&[
                (total >> 21) as u8 & 0x7f,
                (total >> 14) as u8 & 0x7f,
                (total >> 7) as u8 & 0x7f,
                total as u8 & 0x7f,
            ]);
        };
        let mut id3 = Vec::new();
        id3.extend_from_slice(b"ID3");
        id3.extend_from_slice(&[3, 0, 0]);
        synchsafe(body.len() as u32, &mut id3);
        id3.extend_from_slice(&body);

        let mut chunk = Vec::new();
        chunk.extend_from_slice(b"id3 ");
        chunk.extend_from_slice(&(id3.len() as u32).to_le_bytes());
        chunk.extend_from_slice(&id3);
        if id3.len() % 2 == 1 {
            chunk.push(0);
        }
        chunk
    }

    /// A library rooted in a throwaway directory populated with `files`
    /// (relative paths), so browsing tests never touch the real music folder.
    fn library_with(files: &[&str]) -> (crate::api::Library, std::path::PathBuf) {
        let directory = Box::leak(Box::new(tempfile::tempdir().unwrap()));
        let root = directory.path().to_path_buf();
        for file in files {
            write_wav(&root.join(file));
        }
        let library = crate::api::Library::new(
            Some(root.clone()),
            kog_core::db::LibraryDb::open_in_memory().expect("in-memory library"),
        );
        (library, root)
    }

    fn library() -> crate::api::Library {
        library_with(&[]).0
    }

    fn state(auth: AuthMode, token: &str) -> AppState {
        state_with(auth, token, library())
    }

    fn state_with(auth: AuthMode, token: &str, library: crate::api::Library) -> AppState {
        let mut config = ServerConfig {
            enabled: true,
            auth,
            ..ServerConfig::default()
        };
        if !token.is_empty() {
            config.token = token.to_owned();
        }
        AppState::new(config, "9.9.9", streams(), library)
    }

    #[tokio::test]
    async fn media_download_streams_original_bytes_with_length() {
        let (library, root) = library_with(&["large.wav"]);
        let source = root.join("large.wav");
        let bytes = vec![0x5a; 2 * 1024 * 1024];
        std::fs::write(&source, &bytes).unwrap();
        let response = router(state_with(AuthMode::None, "", library))
            .oneshot(
                HttpRequest::builder()
                    .uri(format!("/api/media/download?kind=local&path={}", source.display()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            bytes.len().to_string().as_str()
        );
        let received = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(received.as_ref(), bytes);
    }

    #[test]
    fn range_requests_cover_the_usual_shapes() {
        // Whole-file prefix, suffix, explicit window, and clamping.
        assert_eq!(parse_range("bytes=0-99", 1_000), Some((0, 99)));
        assert_eq!(parse_range("bytes=-100", 1_000), Some((900, 999)));
        assert_eq!(parse_range("bytes=900-", 1_000), Some((900, 999)));
        assert_eq!(parse_range("bytes=0-99999", 1_000), Some((0, 999)));
        // Unsatisfiable or unsupported forms fall back to the whole file.
        assert_eq!(parse_range("bytes=1000-1200", 1_000), None);
        assert_eq!(parse_range("bytes=0-10,20-30", 1_000), None);
        assert_eq!(parse_range("items=0-10", 1_000), None);
        assert_eq!(parse_range("bytes=-", 1_000), None);
        assert_eq!(parse_range("bytes=abc-def", 1_000), None);
        assert_eq!(parse_range("bytes=0-0", 0), None);
    }

    #[tokio::test]
    async fn audio_streams_are_not_cached_by_clients() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("song.mid");
        std::fs::write(&path, b"audio").unwrap();
        for range in [None, Some("bytes=1-3")] {
            let mut headers = HeaderMap::new();
            if let Some(range) = range {
                headers.insert(header::RANGE, HeaderValue::from_static(range));
            }
            let response = serve_file(&path, "audio/aac", &headers).await;
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL).unwrap(),
                "no-store"
            );
        }
    }

    #[test]
    fn stream_locators_match_the_library_key_scheme() {
        let local = StreamQuery {
            kind: "local".to_owned(),
            path: "/music/a.flac".to_owned(),
            entry: String::new(),
            fragment: String::new(),
            codec: None,
            bitrate: None,
        };
        assert_eq!(local.locator(), "/music/a.flac");
        assert_eq!(local.codec(StreamCodec::Aac).unwrap(), StreamCodec::Aac);
        assert_eq!(
            local.codec(StreamCodec::Flac).unwrap(),
            StreamCodec::Flac,
            "an explicit codec wins over the default"
        );

        let archived = StreamQuery {
            kind: "archive".to_owned(),
            path: "/music/pack.zip".to_owned(),
            entry: "Disc/a.wav".to_owned(),
            fragment: "2".to_owned(),
            codec: Some("opus".to_owned()),
            bitrate: None,
        };
        assert_eq!(archived.locator(), "/music/pack.zip::Disc/a.wav#2");
        assert_eq!(archived.codec(StreamCodec::Aac).unwrap(), StreamCodec::Opus);
        assert!(archived.location().is_ok());

        let unknown = StreamQuery {
            codec: Some("mp3".to_owned()),
            ..local
        };
        assert!(unknown.codec(StreamCodec::Aac).is_err());
    }

    #[test]
    fn midi_stream_keys_change_with_synth_for_files_and_archive_members() {
        use kog_audio::settings::MidiEngine;

        for (kind, path, entry) in [
            ("local", "/music/song.MID", ""),
            ("archive", "/music/collection.zip", "Disc/song.mid"),
        ] {
            let query = StreamQuery {
                kind: kind.to_owned(),
                path: path.to_owned(),
                entry: entry.to_owned(),
                fragment: String::new(),
                codec: None,
                bitrate: None,
            };
            let sf2 = stream_key_for_request(&query, StreamCodec::Aac, 192, MidiEngine::RustySynth);
            let opl3 = stream_key_for_request(&query, StreamCodec::Aac, 192, MidiEngine::Opl3Windows);
            assert_ne!(sf2.stem(), opl3.stem());
        }
        let non_midi = StreamQuery {
            kind: "local".to_owned(),
            path: "/music/song.flac".to_owned(),
            entry: String::new(),
            fragment: String::new(),
            codec: None,
            bitrate: None,
        };
        assert_eq!(
            stream_key_for_request(&non_midi, StreamCodec::Aac, 192, MidiEngine::RustySynth),
            stream_key_for_request(&non_midi, StreamCodec::Aac, 192, MidiEngine::Opl3Windows),
        );
    }

    #[test]
    fn unknown_kinds_and_missing_paths_are_rejected() {
        let missing = StreamQuery {
            kind: "local".to_owned(),
            path: String::new(),
            entry: String::new(),
            fragment: String::new(),
            codec: None,
            bitrate: None,
        };
        assert!(missing.location().is_err());
        let bogus = StreamQuery {
            kind: "ftp".to_owned(),
            ..missing
        };
        assert!(bogus.location().is_err());
        let archive_without_member = StreamQuery {
            kind: "archive".to_owned(),
            path: "/music/pack.zip".to_owned(),
            entry: String::new(),
            fragment: String::new(),
            codec: None,
            bitrate: None,
        };
        assert!(archive_without_member.location().is_err());
    }

    #[tokio::test]
    async fn a_cached_track_streams_with_range_support() {
        let state = state(AuthMode::None, "");
        let key = StreamKey::new("/music/cached.flac", StreamCodec::Aac, crate::stream::DEFAULT_BITRATE_KBPS);
        let entry = state.streams.cache().entry_path(&key);
        std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
        std::fs::write(&entry, b"0123456789").unwrap();

        let uri = "/api/stream?kind=local&path=/music/cached.flac";
        let (status, body) = get_json(state.clone(), uri, None).await;
        assert_eq!(status, StatusCode::OK, "cached encode is served directly");

        // Now a range request: the bytes must be exactly the requested window.
        let response = router(state)
            .oneshot(
                HttpRequest::builder()
                    .uri(uri)
                    .header(header::RANGE, "bytes=2-5")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        let content_range = response
            .headers()
            .get(header::CONTENT_RANGE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        assert_eq!(content_range, "bytes 2-5/10");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(bytes.as_ref(), b"2345");
    }

    #[tokio::test]
    async fn a_long_local_mp3_is_seekable_before_any_encode() {
        use std::io::{Seek, Write};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("long audiobook.MP3");
        let mut file = std::fs::File::create(&path).unwrap();
        let size = 220_284_964_u64;
        file.set_len(size).unwrap();
        file.seek(std::io::SeekFrom::Start(size - 4)).unwrap();
        file.write_all(b"TAIL").unwrap();
        drop(file);

        let uri = format!(
            "/api/stream?kind=local&path={}&codec=opus",
            path.to_string_lossy().replace(' ', "%20")
        );
        let app = router(state(AuthMode::Token, "t"));
        let unauthenticated = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri(uri.clone())
                    .header(header::RANGE, format!("bytes={}-", size - 4))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri(uri)
                    .header(header::AUTHORIZATION, "Bearer t")
                    .header(header::RANGE, format!("bytes={}-", size - 4))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "audio/mpeg");
        assert_eq!(response.headers()[header::ACCEPT_RANGES], "bytes");
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "4");
        assert_eq!(
            response.headers()[header::CONTENT_RANGE],
            format!("bytes {}-{}/{size}", size - 4, size - 1)
        );
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(bytes.as_ref(), b"TAIL");
    }

    #[tokio::test]
    async fn a_bad_codec_parameter_is_a_client_error() {
        let (status, body) = get_json(
            state(AuthMode::None, ""),
            "/api/stream?kind=local&path=/music/a.flac&codec=mp3",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("mp3"));

        let (status, _) = get_json(
            state(AuthMode::None, ""),
            "/api/stream?kind=ftp&path=/music/a.flac",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    async fn get_json(
        state: AppState,
        path: &str,
        authorization: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut request = HttpRequest::builder().uri(path);
        if let Some(value) = authorization {
            request = request.header(header::AUTHORIZATION, value);
        }
        let response = router(state)
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .expect("router responds");
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    /// Send a request with an optional JSON body and read the JSON reply.
    async fn request_json(
        state: AppState,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = HttpRequest::builder().method(method).uri(path);
        let body = match body {
            Some(value) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        let response = router(state)
            .oneshot(builder.body(body).unwrap())
            .await
            .expect("router responds");
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    #[tokio::test]
    async fn devices_are_tracked_and_blocking_is_enforced() {
        let state = state(AuthMode::None, "");

        // A request carrying a device id registers the client.
        let response = router(state.clone())
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/health")
                    .header("x-kog-device", "web-test-device")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let (status, body) = get_json(state.clone(), "/api/devices", None).await;
        assert_eq!(status, StatusCode::OK);
        let devices = body["devices"].as_array().unwrap();
        let entry = devices
            .iter()
            .find(|device| device["id"] == "web-test-device")
            .expect("the request's device is listed");
        assert_eq!(entry["blocked"], false);
        assert!(entry["requests"].as_u64().unwrap() >= 1);

        // Blocking refuses the device's API requests with a 403...
        let (status, body) = request_json(
            state.clone(),
            "POST",
            "/api/devices/block",
            Some(serde_json::json!({ "id": "web-test-device", "blocked": true })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], true);
        let blocked = router(state.clone())
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/health")
                    .header("x-kog-device", "web-test-device")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(blocked.status(), StatusCode::FORBIDDEN);

        // ...and unblocking lets it back in. Unknown ids are a bad request.
        let (status, _) = request_json(
            state.clone(),
            "POST",
            "/api/devices/block",
            Some(serde_json::json!({ "id": "web-test-device", "blocked": false })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let allowed = router(state)
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/health")
                    .header("x-kog-device", "web-test-device")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);
        let (status, _) = request_json(
            AppState::new(
                ServerConfig {
                    enabled: true,
                    ..ServerConfig::default()
                },
                "9.9.9",
                streams(),
                library(),
            ),
            "POST",
            "/api/devices/block",
            Some(serde_json::json!({ "id": "never-seen", "blocked": true })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn browsing_lists_the_music_directory_and_stays_inside_it() {
        let (library, root) = library_with(&["Album/one.wav", "Album/two.wav"]);
        let album = root.join("Album");
        // One library, shared through the cloneable AppState.
        let state = state_with(AuthMode::None, "", library);

        let (status, body) = get_json(
            state.clone(),
            &format!("/api/library?path={}", album.display()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let mut names: Vec<&str> = body["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["name"].as_str().unwrap())
            .collect();
        names.sort_unstable();
        assert_eq!(names, ["one.wav", "two.wav"]);
        // Files carry their absolute path, which is what clients then stream.
        assert!(body["files"][0]["path"]
            .as_str()
            .unwrap()
            .starts_with(&root.to_string_lossy().into_owned()));

        // Leaving the music directory is refused: the web tree roots there
        // and cannot climb past it.
        let (status, body) = get_json(state.clone(), "/api/library?path=..", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("outside"));

        let outside = root.parent().unwrap();
        let (status, _) = get_json(
            state,
            &format!("/api/library?path={}", outside.display()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn browsing_reaches_midi_inside_nested_zip_folders() {
        let (library, root) = library_with(&[]);
        let archive = root.join("Sonic.zip");
        let midi = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk\0\0\0\x04\0\xff\x2f\0";
        kog_audio::archive::tests::write_stored_zip(
            &archive,
            &[("Sonic The Hedgehog/Genesis/Green Hill Zone.mid", midi)],
        );
        let state = state_with(AuthMode::None, "", library);
        let mut path = archive.to_string_lossy().into_owned();

        for folder in ["Sonic The Hedgehog", "Genesis"] {
            let (status, body) = get_json(
                state.clone(),
                &format!("/api/library?path={}", path.replace(' ', "%20")),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            let directories = body["directories"].as_array().unwrap();
            assert_eq!(directories.len(), 1);
            assert_eq!(directories[0]["name"], folder);
            path = format!("{path}/{folder}");
            assert_eq!(directories[0]["path"], path);
        }

        let (status, body) = get_json(
            state.clone(),
            &format!("/api/library?path={}", path.replace(' ', "%20")),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let files = body["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["name"], "Green Hill Zone.mid");
        assert_eq!(files[0]["kind"], "archive");
        assert_eq!(files[0]["path"], archive.to_string_lossy().as_ref());
        assert_eq!(
            files[0]["entry"],
            "Sonic The Hedgehog/Genesis/Green Hill Zone.mid"
        );

        let (status, expanded) = request_json(
            state,
            "POST",
            "/api/expand",
            Some(serde_json::json!([{
                "kind": "archive",
                "path": archive,
                "entry": "Sonic The Hedgehog/Genesis/Green Hill Zone.mid",
                "name": "Green Hill Zone.mid"
            }])),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(expanded["tracks"][0][0]["kind"], "archive");
        assert_eq!(expanded["tracks"][0][0]["entry"], files[0]["entry"]);
    }

    #[tokio::test]
    async fn folder_collection_uses_shared_archive_rules_and_music_root() {
        let (library, root) = library_with(&["Album/one.wav"]);
        let archive = root.join("Album/pack.zip");
        let wav = kog_audio::archive::tests::wav_bytes(100);
        kog_audio::archive::tests::write_stored_zip(
            &archive,
            &[("Disc", b""), ("Disc/song.wav", &wav), ("Disc/cover.jpg", b"image")],
        );
        let native = kog_audio::decoder::DecoderRegistry::default()
            .expand_detailed(archive.clone())
            .unwrap();
        assert_eq!(native.sources.len(), 1, "Qt's decoder sees one playable archive member");
        let state = state_with(AuthMode::None, "", library);
        let (status, body) = get_json(
            state.clone(),
            &format!("/api/library/collect?path={}", root.join("Album").display()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let tracks = body["tracks"].as_array().unwrap();
        assert_eq!(tracks.len(), 2, "only playable tracks are collected: {tracks:?}");
        assert!(tracks.iter().any(|entry| entry["name"] == "one.wav"));
        assert!(tracks.iter().any(|entry| entry["entry"] == "Disc/song.wav"));

        let (status, _) = get_json(
            state,
            &format!("/api/library/collect?path={}", root.parent().unwrap().display()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn browsing_expands_folder_playlists_and_drops_gme_companions() {
        let (library, root) = library_with(&["Album/one.wav"]);
        let album = root.join("Album");
        // A plain one-line playlist: its entry replaces the bare .m3u line.
        std::fs::write(album.join("list.m3u"), "one.wav\n").unwrap();
        // GME's companion .m3u (stem matches the sibling audio) is emulator
        // metadata, never a track of its own.
        std::fs::write(album.join("one.m3u"), "# @TITLE x\none.wav::WAV,0,title\n").unwrap();
        // A numbered GME playlist that does not resolve to a path contributes
        // nothing rather than a line that would 400 on stream.
        std::fs::write(album.join("02 track.m3u"), "one.wav::WAV,1,title\n").unwrap();
        let state = state_with(AuthMode::None, "", library);

        let (status, body) = get_json(
            state.clone(),
            &format!("/api/library?path={}", album.display()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let files = body["files"].as_array().unwrap();
        assert_eq!(files.len(), 1, "playlist expanded and deduplicated: {files:?}");
        assert_eq!(files[0]["name"].as_str(), Some("one.wav"));
        assert_eq!(files[0]["kind"].as_str(), Some("local"));
        assert_eq!(files[0]["entry"].as_str(), Some(""));
        assert!(files[0]["fragment"].is_null());
        assert!(
            files
                .iter()
                .all(|file| !file["path"].as_str().unwrap().ends_with(".m3u")),
            "no bare playlist line is offered"
        );

        // With the preference off, playlists found in a folder stay bare files
        // (the current behaviour), but the GME companion is still hidden.
        let (_replaced, root) = library_with(&["Album/one.wav"]);
        let album = root.join("Album");
        std::fs::write(album.join("list.m3u"), "one.wav\n").unwrap();
        std::fs::write(album.join("one.m3u"), "# @TITLE x\n").unwrap();
        let library = crate::api::Library::with_read_playlists_in_folders(
            Some(root.clone()),
            kog_core::db::LibraryDb::open_in_memory().expect("in-memory library"),
            false,
        );
        let state = state_with(AuthMode::None, "", library);
        let (status, body) = get_json(
            state,
            &format!("/api/library?path={}", album.display()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let mut names: Vec<&str> = body["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["name"].as_str().unwrap())
            .collect();
        names.sort_unstable();
        assert_eq!(names, ["list.m3u", "one.wav"]);
    }

    #[tokio::test]
    async fn browsing_expands_subsong_playlists_without_duplicating_the_rom() {
        let (library, root) = library_with(&["Album/game.gbs"]);
        let album = root.join("Album");
        // Eight one-line track playlists, each naming one subsong of the ROM.
        for subsong in 0..8 {
            std::fs::write(
                album.join(format!("{:02} BGM #{:02}.m3u", subsong + 1, subsong + 1)),
                format!("game.gbs::GBS,{subsong},BGM #{:02},1:53,,10\n", subsong + 1),
            )
            .unwrap();
        }
        // The same-stem companion is emulator metadata, never a row of its own.
        std::fs::write(
            album.join("game.m3u"),
            "# @TITLE x\ngame.gbs::GBS,0,BGM #01,1:53,,10\n",
        )
        .unwrap();
        let state = state_with(AuthMode::None, "", library);

        let (status, body) = get_json(
            state,
            &format!("/api/library?path={}", album.display()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let files = body["files"].as_array().unwrap();
        assert_eq!(files.len(), 8, "one row per subsong playlist: {files:?}");
        let mut fragments: Vec<&str> = files
            .iter()
            .map(|file| file["fragment"].as_str().unwrap())
            .collect();
        fragments.sort_unstable();
        assert_eq!(fragments, ["0", "1", "2", "3", "4", "5", "6", "7"]);
        assert!(
            files
                .iter()
                .all(|file| file["path"].as_str().unwrap().ends_with("game.gbs")),
            "every row streams the ROM with a subsong"
        );
        assert!(
            files
                .iter()
                .all(|file| file["name"].as_str().unwrap().starts_with("BGM #")),
            "the playlist title names the row"
        );
        assert!(
            files
                .iter()
                .all(|file| !file["path"].as_str().unwrap().ends_with(".m3u")),
            "the companion and bare playlists are absent"
        );
    }

    #[tokio::test]
    async fn searching_finds_files_by_name() {
        let (library, _root) = library_with(&["Album/one.wav", "Other/two.wav"]);
        let state = state_with(AuthMode::None, "", library);
        let (status, mut body) =
            get_json(state.clone(), "/api/library/search?q=one", None).await;
        assert_eq!(status, StatusCode::OK);
        // The walk is sliced; pull batches until it reports done.
        let generation = body["generation"].as_u64().unwrap();
        let mut names: Vec<String> = search_names(&mut body);
        while body["done"].as_bool() != Some(true) {
            let (status, mut more) = get_json(
                state.clone(),
                &format!("/api/library/search/more?g={generation}"),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            names.extend(search_names(&mut more));
            body = more;
        }
        assert_eq!(names, ["one.wav"]);

        // Words match in any position, like the desktop's search: "moon blue"
        // finds "blue moon.wav" even though no name contains that phrase.
        let (library, _root) = library_with(&["Album/blue moon.wav", "Other/blue.wav"]);
        let state = state_with(AuthMode::None, "", library);
        let (status, mut body) =
            get_json(state.clone(), "/api/library/search?q=moon%20blue", None).await;
        assert_eq!(status, StatusCode::OK);
        let generation = body["generation"].as_u64().unwrap();
        let mut names = search_names(&mut body);
        while body["done"].as_bool() != Some(true) {
            let (status, mut more) = get_json(
                state.clone(),
                &format!("/api/library/search/more?g={generation}"),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            names.extend(search_names(&mut more));
            body = more;
        }
        assert_eq!(names, ["blue moon.wav"]);

        // A whitespace-only query matches nothing and finishes immediately.
        let (status, body) = get_json(state, "/api/library/search?q=%20", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["done"].as_bool(), Some(true));
        assert!(body["results"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn searching_finds_archive_members() {
        // The desktop search lists archives after the filesystem pass and
        // matches member names inside them; the web walk does the same.
        let (library, root) = library_with(&["Other/two.wav"]);
        kog_audio::archive::tests::write_stored_zip(
            &root.join("Sonic pack.zip"),
            &[("Disc/sonic theme.wav", b"0123456789"), ("other.txt", b"x")],
        );
        let _ = &library;
        let state = state_with(AuthMode::None, "", library);
        let (status, mut body) =
            get_json(state.clone(), "/api/library/search?q=sonic", None).await;
        assert_eq!(status, StatusCode::OK);
        let generation = body["generation"].as_u64().unwrap();
        let mut results: Vec<(String, String, bool)> = body["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["name"].as_str().unwrap().to_string(),
                    r["entry"].as_str().unwrap_or_default().to_string(),
                    r["is_dir"].as_bool().unwrap_or(false),
                )
            })
            .collect();
        while body["done"].as_bool() != Some(true) {
            let (status, mut more) = get_json(
                state.clone(),
                &format!("/api/library/search/more?g={generation}"),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            results.extend(
                more["results"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| {
                        (
                            r["name"].as_str().unwrap().to_string(),
                            r["entry"].as_str().unwrap_or_default().to_string(),
                            r["is_dir"].as_bool().unwrap_or(false),
                        )
                    }),
            );
            body = more;
        }
        // The archive container matches its own name, and the member inside
        // it matches too; the unsupported "other.txt" stays out.
        assert!(results.iter().any(|(name, _, dir)| name == "Sonic pack.zip" && *dir));
        assert!(results.iter().any(|(name, entry, dir)| name == "sonic theme.wav"
            && entry == "Disc/sonic theme.wav"
            && !dir));
        assert!(!results.iter().any(|(name, _, _)| name == "other.txt"));
    }

    #[tokio::test]
    async fn searching_a_folder_exposes_its_contents() {
        // "Audiobooks" matches the folder's own name, so everything inside it
        // counts as a match even though the files' names share no word with
        // the query. Files outside the matched folder stay out.
        let (library, _root) =
            library_with(&["Audiobooks/Novel/chapter1.wav", "Other/thing.wav"]);
        let state = state_with(AuthMode::None, "", library);
        let (status, mut body) =
            get_json(state.clone(), "/api/library/search?q=audiobooks", None).await;
        assert_eq!(status, StatusCode::OK);
        let generation = body["generation"].as_u64().unwrap();
        // search_names removes the name fields, so collect the folder results
        // of a batch before pulling its names out.
        let mut dirs: Vec<String> = body["results"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|file| file["is_dir"].as_bool().unwrap_or(false))
            .map(|file| file["name"].as_str().unwrap().to_string())
            .collect();
        let mut names = search_names(&mut body);
        while body["done"].as_bool() != Some(true) {
            let (status, mut more) = get_json(
                state.clone(),
                &format!("/api/library/search/more?g={generation}"),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            names.extend(search_names(&mut more));
            dirs.extend(
                more["results"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|file| file["is_dir"].as_bool().unwrap_or(false))
                    .map(|file| file["name"].as_str().unwrap().to_string()),
            );
            body = more;
        }
        assert!(
            names.contains(&"chapter1.wav".to_owned()),
            "folder contents should surface: {names:?}"
        );
        assert!(
            dirs.contains(&"Audiobooks".to_owned()),
            "the matched folder should appear: {dirs:?}"
        );
        assert!(
            !names.contains(&"thing.wav".to_owned()),
            "files outside the matched folder must not surface: {names:?}"
        );
    }

    fn search_names(body: &mut serde_json::Value) -> Vec<String> {
        body["results"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .map(|file| file["name"].take().as_str().unwrap().to_string())
            .collect()
    }

    #[tokio::test]
    async fn metadata_reports_probed_audio_and_handles_bad_entries() {
        let (library, root) = library_with(&["Album/one.wav"]);
        let path = root.join("Album/one.wav").to_string_lossy().into_owned();
        let state = state_with(AuthMode::None, "", library);

        // The batch is a bare JSON array and keeps its order and length.
        let (status, body) = request_json(
            state.clone(),
            "POST",
            "/api/metadata",
            Some(serde_json::json!([
                { "kind": "local", "path": path, "entry": "", "fragment": null },
                { "kind": "local", "path": "/does/not/exist.wav", "entry": "", "fragment": null },
            ])),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let rows = body.as_array().unwrap();
        assert_eq!(rows.len(), 2, "one row per entry, in order");
        assert_eq!(rows[0]["sampleRate"], 8000);
        assert_eq!(rows[0]["channels"], 1);
        assert!(rows[0]["duration"].as_f64().is_some(), "a WAV has a known duration");
        assert_eq!(rows[0]["title"], serde_json::Value::Null);
        assert_eq!(rows[0]["albumArtist"], serde_json::Value::Null);
        assert_eq!(rows[0]["composer"], serde_json::Value::Null);
        assert_eq!(rows[1], serde_json::Value::Null, "unprobeable entries are null");

        // The single-entry GET shares the shape, and distinguishes 400 from 404.
        let (status, body) = get_json(
            state.clone(),
            &format!("/api/metadata?kind=local&path={path}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["sampleRate"], 8000);

        let (status, _) = get_json(state.clone(), "/api/metadata?kind=local&path=/nope.wav", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = get_json(state.clone(), "/api/metadata?kind=ftp&path=/nope.wav", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Oversized batches are refused rather than pinning a worker.
        let too_many: Vec<serde_json::Value> = (0..1001)
            .map(|_| serde_json::json!({ "kind": "local", "path": "/music/a.wav" }))
            .collect();
        let (status, _) = request_json(
            state,
            "POST",
            "/api/metadata",
            Some(serde_json::Value::Array(too_many)),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn metadata_surfaces_album_artist_and_composer_from_tags() {
        let (library, root) = library_with(&["Album/one.wav", "Album/plain.wav"]);
        let state = state_with(AuthMode::None, "", library);

        // Tag one file with album artist and composer. lofty 0.25 cannot write
        // Id3v2 onto a WAV, so the fixture carries a hand-built ID3v2.3 "id3 "
        // chunk with the TPE2/TCOM frames the desktop's tag path reads.
        let tagged_path = root.join("Album/one.wav");
        let mut tagged = std::fs::read(&tagged_path).unwrap();
        if tagged.len() % 2 == 1 {
            tagged.push(0);
        }
        tagged.extend_from_slice(id3_chunk("Session Orchestra", "Ada Lane").as_slice());
        std::fs::write(&tagged_path, tagged).unwrap();

        let tagged_file = root.join("Album/one.wav").to_string_lossy().into_owned();
        let (_, tagged_row) = request_json(
            state.clone(),
            "POST",
            "/api/metadata",
            Some(serde_json::json!([
                { "kind": "local", "path": tagged_file, "entry": "", "fragment": null },
            ])),
        )
        .await;
        let rows = tagged_row.as_array().unwrap();
        assert_eq!(rows[0]["albumArtist"], "Session Orchestra");
        assert_eq!(rows[0]["composer"], "Ada Lane");

        let plain_file = root.join("Album/plain.wav").to_string_lossy().into_owned();
        let (_, plain_row) = request_json(
            state.clone(),
            "POST",
            "/api/metadata",
            Some(serde_json::json!([
                { "kind": "local", "path": plain_file, "entry": "", "fragment": null },
            ])),
        )
        .await;
        let rows = plain_row.as_array().unwrap();
        assert_eq!(rows[0]["albumArtist"], serde_json::Value::Null);
        assert_eq!(rows[0]["composer"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn playlists_can_be_created_appended_and_deleted() {
        let state = state(AuthMode::None, "");
        let (status, body) = request_json(
            state.clone(),
            "POST",
            "/api/playlists",
            Some(serde_json::json!({ "name": "Road trip" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = body["id"].as_i64().expect("a new playlist has an id");
        assert!(id > 0, "Favorites is 0 and never created by clients");

        let (status, body) = request_json(
            state.clone(),
            "POST",
            &format!("/api/playlists/{id}/entries"),
            Some(serde_json::json!({
                "entries": [
                    { "kind": "local", "path": "/music/a.flac" },
                    { "kind": "archive", "path": "/music/pack.zip", "entry": "Disc/b.wav", "fragment": "2" },
                ]
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["added"], 2);

        let (status, body) = get_json(state.clone(), &format!("/api/playlists/{id}"), None).await;
        assert_eq!(status, StatusCode::OK);
        let entries = body["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["path"], "/music/a.flac");
        assert_eq!(entries[1]["entry"], "Disc/b.wav");
        assert_eq!(entries[1]["fragment"], "2");

        let (_, body) = get_json(state.clone(), "/api/playlists", None).await;
        assert!(body["playlists"]
            .as_array()
            .unwrap()
            .iter()
            .any(|playlist| playlist["id"] == id && playlist["entryCount"] == 2));

        // Favorites is managed through stars, not by appending.
        let (status, _) = request_json(
            state.clone(),
            "POST",
            "/api/playlists/0/entries",
            Some(serde_json::json!({ "entries": [{ "kind": "local", "path": "/music/a.flac" }] })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _) = request_json(
            state.clone(),
            "DELETE",
            &format!("/api/playlists/{id}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (_, body) = get_json(state, "/api/playlists", None).await;
        assert!(!body["playlists"]
            .as_array()
            .unwrap()
            .iter()
            .any(|playlist| playlist["id"] == id));
    }

    #[tokio::test]
    async fn stars_round_trip_with_the_locator_scheme() {
        let state = state(AuthMode::None, "");
        let entry = serde_json::json!({
            "kind": "archive",
            "path": "/music/pack.zip",
            "entry": "Disc/a.wav",
            "fragment": "2",
        });

        let mut starred = entry.clone();
        starred["starred"] = serde_json::json!(true);
        let (status, _) = request_json(state.clone(), "POST", "/api/stars", Some(starred)).await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = get_json(state.clone(), "/api/stars", None).await;
        assert_eq!(status, StatusCode::OK);
        let entries = body["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 1, "exactly the one entry that was starred");
        assert_eq!(entries[0]["path"], "/music/pack.zip");
        assert_eq!(entries[0]["entry"], "Disc/a.wav");
        assert_eq!(entries[0]["fragment"], "2");

        let mut unstarred = entry;
        unstarred["starred"] = serde_json::json!(false);
        let (status, _) = request_json(state.clone(), "POST", "/api/stars", Some(unstarred)).await;
        assert_eq!(status, StatusCode::OK);
        let (_, body) = get_json(state, "/api/stars", None).await;
        assert!(body["entries"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn health_and_version_are_open() {
        let (status, body) = get_json(state(AuthMode::Token, "t"), "/api/health", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], true);

        let (status, body) = get_json(state(AuthMode::Token, "t"), "/api/version", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["version"], "9.9.9");
        assert_eq!(body["api"], 1);
    }

    #[tokio::test]
    async fn protected_routes_require_the_token() {
        let (status, body) = get_json(state(AuthMode::Token, "t"), "/api/codecs", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["ok"], false);

        let (status, _) = get_json(state(AuthMode::Token, "t"), "/api/codecs", Some("Bearer t")).await;
        assert_eq!(status, StatusCode::OK);

        let (status, _) = get_json(
            state(AuthMode::Token, "a b/c+"),
            "/api/codecs?token=a+b%2Fc%2B",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn the_challenge_header_names_both_schemes() {
        let response = router(state(AuthMode::Token, "t"))
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/codecs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let challenge = response
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(challenge.contains("Bearer"));
        assert!(challenge.contains("Basic"));
    }

    #[tokio::test]
    async fn codecs_are_advertised_for_clients() {
        let (_, body) = get_json(state(AuthMode::None, ""), "/api/codecs", None).await;
        let ids: Vec<&str> = body["codecs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|codec| codec["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["aac", "opus", "flac"]);
    }

    #[tokio::test]
    async fn the_running_config_never_leaks_the_token() {
        let (_, body) = get_json(state(AuthMode::Token, "super-secret"), "/api/config", Some("Bearer super-secret")).await;
        let text = body.to_string();
        assert!(!text.contains("super-secret"), "config response leaked the token: {text}");
        assert_eq!(body["auth"], "token");
    }
}
