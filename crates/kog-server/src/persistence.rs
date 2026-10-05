//! Durable client state in the same SQLite database as the shared library.
//! Host preferences (including server credentials) are never exposed here.
use crate::{
    api::Library,
    routes::{AppState, bad_request},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use kog_core::db::StateWriteError;
use serde::Deserialize;
use serde_json::{Value, json};

fn valid_key(namespace: &str, id: &str) -> bool {
    matches!(namespace, "sessions" | "session-ui" | "client-preferences")
        && !id.is_empty()
        && id.len() <= 256
        && !id.chars().any(char::is_control)
}

pub async fn load(
    State(state): State<AppState>,
    Path((namespace, id)): Path<(String, String)>,
) -> Response {
    if !valid_key(&namespace, &id) {
        return bad_request("Invalid client state key");
    }
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || read(&library, &namespace, &id)).await;
    match result {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":error})),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":error.to_string()})),
        )
            .into_response(),
    }
}

fn read(library: &Library, namespace: &str, id: &str) -> Result<Value, String> {
    match library.db().load_state(namespace, id)? {
        Some(saved) => {
            let value: Value = serde_json::from_str(&saved.value)
                .map_err(|e| format!("Invalid saved state: {e}"))?;
            Ok(json!({"revision":saved.revision,"value":value}))
        }
        None => Ok(json!({"revision":0,"value":null})),
    }
}

#[derive(Deserialize)]
pub struct Save {
    pub expected_revision: i64,
    pub value: Value,
}

pub async fn save(
    State(state): State<AppState>,
    Path((namespace, id)): Path<(String, String)>,
    Json(input): Json<Save>,
) -> Response {
    if !valid_key(&namespace, &id) || !(0..i64::MAX).contains(&input.expected_revision) {
        return bad_request("Invalid client state key or revision");
    }
    if !input.value.is_object()
        || (namespace == "sessions"
            && (input.value["session_id"].as_str() != Some(&id) || input.value["version"] != 1))
    {
        return bad_request("Invalid client checkpoint");
    }
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        library.db().save_state_checked(
            &namespace,
            &id,
            &input.value.to_string(),
            input.expected_revision,
        )
    })
    .await;
    match result {
        Ok(Ok(revision)) => Json(json!({"revision":revision})).into_response(),
        Ok(Err(StateWriteError::Conflict)) => (StatusCode::CONFLICT, Json(json!({
            "error":"This session was saved by another instance. Reload it before saving again; your current changes have not overwritten the saved session.",
            "code":"state_conflict",
        }))).into_response(),
        Ok(Err(error)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":error.to_string()}))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":error.to_string()}))).into_response(),
    }
}
