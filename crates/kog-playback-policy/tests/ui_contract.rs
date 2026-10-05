use serde_json::{Value, json};

#[test]
fn expected_ui_contract_through_wire_and_direct_backend() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/ui-contract/playlist.json")).unwrap();
    let mut direct = kog_playback_policy::bridge::PolicyState::default();
    let mut state = None;
    let mut load = Value::Null;
    let mut save = Value::Null;
    for (index, step) in fixture["steps"].as_array().unwrap().iter().enumerate() {
        let command = if let Some(entries) = step.get("load") {
            json!({"op":"workspace","command":{"op":"loaded","key":load["key"],"generation":load["generation"],"entries":entries}})
        } else if step.get("save_ok").is_some() {
            json!({"op":"workspace","command":{"op":"saved","key":save["key"],"revision":save["revision"]}})
        } else {
            step["command"].clone()
        };
        let native =
            serde_json::to_value(direct.apply(serde_json::from_value(command.clone()).unwrap()))
                .unwrap();
        let reply: Value = serde_json::from_str(
            &kog_playback_policy::bridge::dispatch_json(
                &json!({"state":state,"command":command}).to_string(),
            )
            .unwrap(),
        )
        .unwrap();
        state = reply["state"].as_str().map(str::to_owned);
        for (path, expected) in step["expect"].as_object().unwrap() {
            let pointer = format!("/{}", path.replace('.', "/"));
            assert_eq!(
                reply.pointer(&pointer),
                Some(expected),
                "wire step {index}, {path}"
            );
            assert_eq!(
                native.pointer(&pointer),
                Some(expected),
                "native step {index}, {path}"
            );
        }
        match reply["workspace_effect"]["action"].as_str() {
            Some("load") => load = reply["workspace_effect"].clone(),
            Some("save") => save = reply["workspace_effect"].clone(),
            _ => (),
        }
    }
}

#[test]
fn selection_follows_visible_order_and_keeps_the_anchor_when_range_shrinks() {
    use kog_playback_policy::selection::{Command, Gesture, Selection};
    let mut state = Selection::default();
    let order = [8, 2, 5, 1];
    state.apply(
        Command::Choose {
            index: 2,
            gesture: Gesture::Replace,
        },
        10,
        &order,
    );
    state.apply(
        Command::Choose {
            index: 1,
            gesture: Gesture::Range,
        },
        10,
        &order,
    );
    assert_eq!(state.indices, [1, 2, 5]);
    state.apply(
        Command::Choose {
            index: 8,
            gesture: Gesture::Range,
        },
        10,
        &order,
    );
    assert_eq!(state.indices, [2, 8]);
    assert_eq!(state.anchor, Some(2));
    state.apply(
        Command::Choose {
            index: 999,
            gesture: Gesture::Toggle,
        },
        10,
        &order,
    );
    assert_eq!(state.indices, [2, 8]);
}

#[test]
fn application_sessions_match_across_native_and_wire_ports() {
    use kog_playback_policy::{
        RepeatMode, ShuffleMode,
        session::{Session, dispatch_json},
    };
    use std::collections::BTreeMap;
    fn resolve(value: &Value, captures: &BTreeMap<String, Value>) -> Value {
        match value {
            Value::String(s) if s.starts_with('@') => captures.get(&s[1..]).unwrap().clone(),
            Value::Array(a) => Value::Array(a.iter().map(|v| resolve(v, captures)).collect()),
            Value::Object(o) => Value::Object(
                o.iter()
                    .map(|(k, v)| (k.clone(), resolve(v, captures)))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/ui-contract/session.json")).unwrap();
    let mut native = BTreeMap::<String, Session<Value>>::new();
    let mut wire = BTreeMap::<String, String>::new();
    let mut captures = BTreeMap::new();
    for (index, step) in fixture["steps"].as_array().unwrap().iter().enumerate() {
        let id = step["session"].as_str().unwrap();
        let direct = native
            .entry(id.to_owned())
            .or_insert_with(|| Session::new(id, 123, ShuffleMode::Off, RepeatMode::Off));
        let command = step.get("command").map(|v| resolve(v, &captures));
        let restore = step.get("restore").map(|v| resolve(v, &captures));
        if let Some(restore) = restore.clone() {
            direct.restore(restore, |v| Ok(v.clone())).unwrap();
        }
        let effects = command
            .clone()
            .map(|c| direct.dispatch(serde_json::from_value(c).unwrap()))
            .unwrap_or_default();
        let native_reply = json!({"snapshot":direct.snapshot(),"effects":effects,"checkpoint":direct.checkpoint()});
        let reply:Value=serde_json::from_str(&dispatch_json(&json!({"state":wire.get(id),"session_id":id,"incarnation":123,"command":command,"restore":restore}).to_string()).unwrap()).unwrap();
        wire.insert(id.to_owned(), reply["state"].as_str().unwrap().into());
        for (path, expected) in step["expect"].as_object().unwrap() {
            let pointer = format!("/{}", path.replace('.', "/"));
            assert_eq!(
                reply.pointer(&pointer),
                Some(expected),
                "wire step {index}: {path}"
            );
            assert_eq!(
                native_reply.pointer(&pointer),
                Some(expected),
                "native step {index}: {path}"
            );
        }
        assert_eq!(
            native_reply["snapshot"], reply["snapshot"],
            "full snapshot at step {index}"
        );
        if let Some(values) = step["capture"].as_object() {
            for (name, path) in values {
                let pointer = format!("/{}", path.as_str().unwrap().replace('.', "/"));
                captures.insert(name.clone(), reply.pointer(&pointer).unwrap().clone());
            }
        }
    }
}
