//! HTTP surface: health/version, the authenticated API root, and (soon) the
//! streaming endpoints. Handlers stay thin; the interesting logic lives in
//! `config` and `auth` so it can be tested without a socket.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tokio::sync::RwLock;

use crate::auth::{AuthError, ConfigView, authorize};
use crate::{ServerConfig, StreamCodec};

/// Shared server state. The config is behind a lock so the Preferences pane can
/// change it while the server runs.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<ServerConfig>>,
    pub version: &'static str,
}

impl AppState {
    pub fn new(config: ServerConfig, version: &'static str) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            version,
        }
    }
}

/// Build the router. Kept separate from `serve` so tests can drive it
/// in-process with `tower::ServiceExt::oneshot`.
pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/codecs", get(codecs))
        .route("/api/config", get(read_config))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/api/health", get(health))
        .route("/api/version", get(version))
        .merge(protected)
        .with_state(state)
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

/// Bind and serve until cancelled. TLS is added in a later step; the config
/// validation already refuses unsafe combinations.
pub async fn serve(state: AppState) -> Result<(), String> {
    let (address, enabled) = {
        let config = state.config.read().await;
        (config.socket_address(), config.enabled)
    };
    if !enabled {
        return Err("the API server is disabled".to_owned());
    }
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| format!("binding {address}: {error}"))?;
    axum::serve(listener, router(state))
        .await
        .map_err(|error| format!("serving on {address}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthMode;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn state(auth: AuthMode, token: &str) -> AppState {
        let mut config = ServerConfig {
            enabled: true,
            auth,
            ..ServerConfig::default()
        };
        if !token.is_empty() {
            config.token = token.to_owned();
        }
        AppState::new(config, "9.9.9")
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
