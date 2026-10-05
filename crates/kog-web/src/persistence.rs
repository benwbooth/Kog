//! One ordered writer per SQLite record. Slow replies cannot reorder saves.
use super::*;
use serde_json::{Value, json};

#[derive(Default)]
struct Pending {
    base: String,
    id: String,
    header: Option<String>,
    epoch: u64,
    revision: i64,
    ready: bool,
    running: bool,
    conflict: bool,
    latest: Option<Value>,
    saved: Option<Value>,
}

#[derive(Clone, Copy)]
pub struct Writer {
    namespace: &'static str,
    state: StoredValue<Pending>,
    error: Callback<String>,
}

impl Writer {
    pub fn new(namespace: &'static str, error: Callback<String>) -> Self {
        Self {
            namespace,
            state: StoredValue::new(Pending::default()),
            error,
        }
    }

    pub fn ready(self, base: &str) -> bool {
        self.state.with_value(|s| s.ready && s.base == base)
    }

    pub fn dirty(self) -> bool {
        self.state.with_value(|s| s.running || s.latest.is_some())
    }

    /// None outside the tuple means this connection was already restored.
    pub async fn connect(
        self,
        base: String,
        id: String,
        header: Option<String>,
    ) -> Result<Option<Option<Value>>, String> {
        if self
            .state
            .with_value(|s| s.ready && s.base == base && s.id == id)
        {
            self.state.update_value(|s| s.header = header);
            self.flush();
            return Ok(None);
        }
        if self.dirty() {
            return Err("Wait for the current session to save before changing servers. If another instance changed it, reload to use that saved session.".into());
        }
        self.state.update_value(|s| {
            let epoch = s.epoch.wrapping_add(1);
            *s = Pending {
                base: base.clone(),
                id: id.clone(),
                header: header.clone(),
                epoch,
                ..Pending::default()
            };
        });
        let epoch = self.state.with_value(|s| s.epoch);
        let result = session::request(
            "GET",
            format!("{base}/api/state/{}/{}", self.namespace, url_encode(&id)),
            header,
            None,
        )
        .await?;
        if self.state.with_value(|s| s.epoch != epoch) {
            return Err("Connection changed while loading the session".into());
        }
        let revision = result["revision"]
            .as_i64()
            .ok_or("Missing saved-state revision")?;
        let value = result.get("value").filter(|v| !v.is_null()).cloned();
        self.state.update_value(|s| {
            s.revision = revision;
            s.saved = value.clone();
            s.ready = true;
        });
        Ok(Some(value))
    }

    pub fn disable(self) {
        self.state.update_value(|s| s.ready = false);
    }

    pub fn save(self, value: Value) {
        self.state.update_value(|s| {
            if s.ready && (s.running || s.saved.as_ref() != Some(&value)) {
                s.latest = Some(value);
            }
        });
        self.flush();
    }

    fn flush(self) {
        let run = self
            .state
            .with_value(|s| s.ready && !s.running && !s.conflict && s.latest.is_some());
        if !run {
            return;
        }
        self.state.update_value(|s| s.running = true);
        let epoch = self.state.with_value(|s| s.epoch);
        leptos::task::spawn_local(async move {
            loop {
                if self.state.with_value(|s| s.epoch != epoch) {
                    return;
                }
                let next = {
                    let mut state = self.state.write_value();
                    match state.latest.take() {
                        Some(value) if state.saved.as_ref() != Some(&value) => Some((
                            state.base.clone(),
                            state.id.clone(),
                            state.header.clone(),
                            state.revision,
                            value,
                        )),
                        _ => {
                            state.running = false;
                            None
                        }
                    }
                };
                let Some((base, id, header, revision, value)) = next else {
                    return;
                };
                let url = format!("{base}/api/state/{}/{}", self.namespace, url_encode(&id));
                // A lost response may have committed. Retry precisely the same
                // checkpoint/revision so SQLite can acknowledge it idempotently
                // before we send the newer coalesced checkpoint.
                let result = loop {
                    let mut request = Request::put(&url).header("X-Kog-Device", &device_id());
                    let header = self
                        .state
                        .with_value(|s| s.header.clone())
                        .or(header.clone());
                    if let Some(header) = header {
                        request = request.header("Authorization", &header);
                    }
                    let result = async {
                        let response = request
                            .json(&json!({"expected_revision":revision,"value":value}))
                            .map_err(|e| (false, e.to_string()))?
                            .send()
                            .await
                            .map_err(|e| (false, e.to_string()))?;
                        if !response.ok() {
                            let conflict = response.status() == 409;
                            return Err((conflict, error_text(response).await));
                        }
                        let reply = response
                            .json::<Value>()
                            .await
                            .map_err(|e| (false, e.to_string()))?;
                        reply["revision"]
                            .as_i64()
                            .ok_or((false, "Missing saved-state revision".into()))
                    }
                    .await;
                    if self.state.with_value(|s| s.epoch != epoch) {
                        return;
                    }
                    match &result {
                        Err((false, error)) => {
                            self.error.run(error.clone());
                            let _ = sleep_ms(5000).await;
                        }
                        _ => break result,
                    }
                };
                if self.state.with_value(|s| s.epoch != epoch) {
                    return;
                }
                match result {
                    Ok(revision) => self.state.update_value(|s| {
                        s.revision = revision;
                        s.saved = Some(value);
                    }),
                    Err((conflict, error)) => {
                        self.state.update_value(|s| {
                            if s.latest.is_none() {
                                s.latest = Some(value);
                            }
                            s.running = false;
                            s.conflict = conflict;
                        });
                        self.error.run(error);
                        return;
                    }
                }
            }
        });
    }
}
