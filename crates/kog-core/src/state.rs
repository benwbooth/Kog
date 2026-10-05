//! SQLite-backed application state and one-time import of legacy files.
use std::path::Path;

use crate::db::LibraryDb;

pub fn load_or_import(
    db: &LibraryDb,
    namespace: &str,
    key: &str,
    legacy: Option<&Path>,
) -> Result<Option<crate::db::StoredState>, String> {
    if let Some(saved) = db.load_state(namespace, key)? {
        return Ok(Some(saved));
    }
    let Some(path) = legacy else { return Ok(None) };
    let value = match std::fs::read_to_string(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("reading legacy state {}: {error}", path.display())),
    };
    db.import_state(namespace, key, &value).map(Some)
}

/// A checkpoint writer remembers exactly which revision it restored. SQLite's
/// compare-and-swap prevents a second process from overwriting a newer save.
pub struct StateCursor {
    namespace: String,
    key: String,
    revision: i64,
    value: Option<String>,
    error: Option<String>,
}

impl StateCursor {
    pub fn open(db: &LibraryDb, namespace: &str, key: &str, legacy: Option<&Path>) -> Self {
        let mut cursor = Self {
            namespace: namespace.into(),
            key: key.into(),
            revision: 0,
            value: None,
            error: None,
        };
        match load_or_import(db, namespace, key, legacy) {
            Ok(Some(saved)) => {
                cursor.revision = saved.revision;
                cursor.value = Some(saved.value);
            }
            Ok(None) => {}
            Err(error) => cursor.error = Some(error),
        }
        cursor
    }

    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn reject(&mut self, error: String) {
        self.error = Some(error);
    }

    pub fn save(&mut self, db: &LibraryDb, value: &str) -> Result<(), String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if self.value.as_deref() == Some(value) {
            return Ok(());
        }
        self.revision = db
            .save_state_checked(&self.namespace, &self.key, value, self.revision)
            .map_err(|e| e.to_string())?;
        self.value = Some(value.into());
        Ok(())
    }
}

/// Preferences use individual SQLite keys. The old files are migration input
/// only and remain untouched for downgrade/recovery.
pub fn load_preference(key: &str, legacy: Option<&Path>) -> Result<Option<String>, String> {
    load_or_import(&LibraryDb::open()?, "preferences", key, legacy)
        .map(|value| value.map(|saved| saved.value))
}

pub fn save_preference(key: &str, value: &str) -> Result<(), String> {
    LibraryDb::open()?.put_state("preferences", key, value)
}

/// Native mobile preference port. Callers pass their private on-device database
/// path; this function is exposed through JNI/C only, never through HTTP.
pub fn preferences_request(input: serde_json::Value) -> Result<serde_json::Value, String> {
    use serde_json::{Value, json};
    let path = Path::new(input["database"].as_str().ok_or("Missing database path")?);
    if !path.is_absolute() {
        return Err("The database path must be absolute".into());
    }
    crate::db::configure_device_database(path)?;
    let db = LibraryDb::open_at(path)?;
    let namespace = "mobile-preferences";
    match input["op"].as_str().ok_or("Missing preference operation")? {
        "get" => {
            let key = input["key"].as_str().ok_or("Missing preference key")?;
            match db.load_state(namespace, key)? {
                Some(saved) => Ok(
                    json!({"found":true,"value":serde_json::from_str::<Value>(&saved.value).map_err(|e| e.to_string())?}),
                ),
                None => Ok(json!({"found":false})),
            }
        }
        "import" => {
            let key = input["key"].as_str().ok_or("Missing preference key")?;
            let saved = db.import_state(namespace, key, &input["value"].to_string())?;
            Ok(
                json!({"found":true,"value":serde_json::from_str::<Value>(&saved.value).map_err(|e| e.to_string())?}),
            )
        }
        "default" => {
            let key = input["key"].as_str().ok_or("Missing preference key")?;
            db.write_transaction(|db| {
                if let Some(saved) = db.load_state(namespace, key)? {
                    let value: Value =
                        serde_json::from_str(&saved.value).map_err(|e| e.to_string())?;
                    if !value.is_null() {
                        return Ok(json!({"value":value}));
                    }
                }
                db.put_state(namespace, key, &input["value"].to_string())?;
                Ok(json!({"value":input["value"]}))
            })
        }
        "all" => {
            let mut values = serde_json::Map::new();
            for (key, saved) in db.list_state(namespace)? {
                values.insert(
                    key,
                    serde_json::from_str(&saved.value).map_err(|e| e.to_string())?,
                );
            }
            Ok(Value::Object(values))
        }
        "write" => {
            let values = input["values"]
                .as_object()
                .ok_or("Missing preference changes")?;
            db.write_transaction(|db| {
                if input["clear"].as_bool().unwrap_or(false) {
                    for (key, _) in db.list_state(namespace)? {
                        db.put_state(namespace, &key, "null")?;
                    }
                }
                for (key, value) in values {
                    db.put_state(namespace, key, &value.to_string())?;
                }
                Ok(())
            })?;
            Ok(json!({"ok":true}))
        }
        _ => Err("Unknown preference operation".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn migration_restarts_conflicts_and_lost_replies_preserve_saved_state() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("kog.db");
        let legacy = temp.path().join("session.json");
        std::fs::write(&legacy, "legacy checkpoint").unwrap();
        let first = LibraryDb::open_at(&path).unwrap();
        let second = LibraryDb::open_at(&path).unwrap();
        let mut a = StateCursor::open(&first, "sessions", "same", Some(&legacy));
        let mut b = StateCursor::open(&second, "sessions", "same", Some(&legacy));
        assert_eq!(a.value(), Some("legacy checkpoint"));
        a.save(&first, "new checkpoint").unwrap();
        assert!(b.save(&second, "stale checkpoint").is_err());
        assert_eq!(
            second
                .save_state_checked("sessions", "same", "new checkpoint", 1)
                .unwrap(),
            2
        );
        std::fs::write(&legacy, "old app ran again").unwrap();
        let restarted = LibraryDb::open_at(&path).unwrap();
        let mut loaded = StateCursor::open(&restarted, "sessions", "same", Some(&legacy));
        assert_eq!(loaded.value(), Some("new checkpoint"));
        loaded.reject("invalid checkpoint".into());
        assert!(loaded.save(&restarted, "empty replacement").is_err());
        assert_eq!(
            restarted
                .load_state("sessions", "same")
                .unwrap()
                .unwrap()
                .value,
            "new checkpoint"
        );
        assert_eq!(
            std::fs::read_to_string(legacy).unwrap(),
            "old app ran again"
        );
    }

    #[test]
    fn mobile_preferences_merge_deltas_import_once_and_keep_tombstones() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.sqlite");
        let call = |mut value: Value| {
            value["database"] = json!(path);
            preferences_request(value).unwrap()
        };
        assert_eq!(call(json!({"op":"get","key":"codec"}))["found"], false);
        assert_eq!(
            call(json!({"op":"import","key":"codec","value":"aac"}))["value"],
            "aac"
        );
        call(json!({"op":"write","values":{"codec":"flac","volume":0.25}}));
        call(json!({"op":"write","values":{"root":"/music"}}));
        assert_eq!(
            call(json!({"op":"import","key":"codec","value":"old"}))["value"],
            "flac"
        );
        call(json!({"op":"write","values":{"codec":null}}));
        assert_eq!(
            call(json!({"op":"import","key":"codec","value":"resurrected"}))["value"],
            Value::Null
        );
        assert_eq!(call(json!({"op":"get","key":"volume"}))["value"], 0.25);
        assert_eq!(
            call(json!({"op":"default","key":"id","value":"first"}))["value"],
            "first"
        );
        assert_eq!(
            call(json!({"op":"default","key":"id","value":"second"}))["value"],
            "first"
        );
        // Independent native clients keep each other's per-key changes.
        std::thread::scope(|scope| {
            for client in 0..4 {
                let path = &path;
                scope.spawn(move || {
                    for index in 0..20 {
                        preferences_request(json!({"database":path,"op":"write","values":{format!("client-{client}"):index}})).unwrap();
                    }
                });
            }
        });
        let all = call(json!({"op":"all"}));
        for client in 0..4 {
            assert_eq!(all[format!("client-{client}")], 19);
        }
        assert_eq!(all["root"], "/music");
    }
}
