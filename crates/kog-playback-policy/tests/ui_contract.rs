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
