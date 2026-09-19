//! HTTP surface: health/version, the authenticated API root, and (soon) the
//! streaming endpoints. Handlers stay thin; the interesting logic lives in
//! `config` and `auth` so it can be tested without a socket.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
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
            crate::service::StreamService::default_encoder(),
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
        use kog_audio::playlist::PlaylistLocation;
        if self.path.trim().is_empty() {
            return Err("a track path is required".to_owned());
        }
        match self.kind.as_str() {
            "local" => Ok(PlaylistLocation::Local(std::path::PathBuf::from(&self.path))),
            "archive" => {
                if self.entry.trim().is_empty() {
                    return Err("an archive member name is required".to_owned());
                }
                Ok(PlaylistLocation::Archive {
                    archive_path: std::path::PathBuf::from(&self.path),
                    entry_name: self.entry.clone(),
                })
            }
            "remote" => Ok(PlaylistLocation::Remote(self.path.clone())),
            other => Err(format!("unknown track kind: {other}")),
        }
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

/// Serve one track: the cached encode when we have it (with `Range`, so
/// clients can seek), otherwise encode on the fly while teeing into the cache.
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
    let bitrate = query.bitrate.unwrap_or(crate::stream::DEFAULT_BITRATE_KBPS);
    let key = StreamKey::new(query.locator(), codec, bitrate);

    // Cache lookup, decoder resolution and the encoder check all touch the
    // filesystem, so they run on a blocking thread; failures come back as a
    // real HTTP error instead of a 200 with an empty body.
    let streams = state.streams.clone();
    let opened = tokio::task::spawn_blocking(move || streams.open(entry, key))
        .await
        .unwrap_or_else(|error| Err(format!("stream setup failed: {error}")));

    match opened {
        Ok(crate::service::StreamSource::Cached(path)) => {
            serve_cached(&path, codec, &headers).await
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

/// Serve a finished encode, honouring a single `Range` request so browsers can
/// seek within a cached track.
async fn serve_cached(
    path: &std::path::Path,
    codec: StreamCodec,
    headers: &axum::http::HeaderMap,
) -> Response {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return bad_request("the cached stream is unavailable");
    };
    let Ok(total) = file.metadata().await.map(|metadata| metadata.len()) else {
        return bad_request("the cached stream is unavailable");
    };
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
                .insert(header::CONTENT_TYPE, HeaderValue::from_static(codec.content_type()));
            response
                .headers_mut()
                .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
            response.headers_mut().insert(
                header::CONTENT_LENGTH,
                HeaderValue::from(total),
            );
            return response;
        }
    };

    let length = end - start + 1;
    if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return bad_request("the cached stream could not be read");
    }
    let body = axum::body::Body::from_stream(
        tokio_util::io::ReaderStream::new(file.take(length)),
    );
    let mut response = Response::new(body);
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(codec.content_type()));
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
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
            // Always revalidate, and answer 304 while the bytes are unchanged.
            let etag = build_etag();
            let unchanged = headers
                .get(header::IF_NONE_MATCH)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.split(',').any(|part| part.trim() == etag));
            let mut response = if unchanged {
                let mut response = Response::new(axum::body::Body::empty());
                *response.status_mut() = StatusCode::NOT_MODIFIED;
                response
            } else {
                let body = axum::body::Body::from(file.contents().to_vec());
                let mut response = Response::new(body);
                response
                    .headers_mut()
                    .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
                response
            };
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
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
        return axum::serve(listener, app)
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
            std::path::PathBuf::from("ffmpeg"),
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

        // Leaving the root is refused rather than clamped to it.
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
        let (status, body) =
            get_json(state.clone(), "/api/library/search?q=one", None).await;
        assert_eq!(status, StatusCode::OK);
        let names: Vec<&str> = body["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["one.wav"]);

        // An empty query is a client error, not a full listing.
        let (status, _) = get_json(state, "/api/library/search?q=%20", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
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
