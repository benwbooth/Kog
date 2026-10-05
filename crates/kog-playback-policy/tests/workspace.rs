use kog_playback_policy::workspace::{
    CloseChoice, Command as C, Effect, QUEUE_TAB, QueueAction, Workspace,
};
use kog_playback_policy::{PlaybackDecision, PlaybackOrder, RepeatMode, ShuffleMode};
use serde_json::json;

fn open(w: &mut Workspace, key: &str, id: i64) -> u64 {
    match w
        .apply(C::Open {
            key: key.into(),
            scope: "test".into(),
            playlist_id: id,
            name: key.into(),
            readonly: false,
        })
        .unwrap()
    {
        Effect::Load { generation, .. } => generation,
        _ => panic!("new tab should request a load"),
    }
}
fn loaded() -> Workspace {
    let mut w = Workspace::default();
    let generation = open(&mut w, "album", 4);
    w.apply(C::Loaded {
        key: "album".into(),
        generation,
        entries: vec![
            json!({"path":"same"}),
            json!({"path":"middle"}),
            json!({"path":"same"}),
        ],
    })
    .unwrap();
    w
}

#[test]
fn dragging_draft_rows_preserves_selection_and_undo() {
    let mut workspace = loaded();
    workspace
        .apply(C::Select {
            indices: vec![0, 2],
        })
        .unwrap();
    workspace
        .apply_ui(C::Move { target: usize::MAX }, 0, 0)
        .unwrap();
    assert_eq!(
        workspace.snapshot().entries,
        vec![
            json!({"path":"middle"}),
            json!({"path":"same"}),
            json!({"path":"same"})
        ]
    );
    assert_eq!(workspace.snapshot().selected, vec![1, 2]);
    workspace.apply(C::Undo).unwrap();
    assert_eq!(workspace.snapshot().selected, vec![0, 2]);
    assert!(!workspace.snapshot().tabs[1].dirty);
    // Dropping an already contiguous selection back into itself is a no-op.
    workspace
        .apply(C::Select {
            indices: vec![0, 1],
        })
        .unwrap();
    workspace.apply_ui(C::Move { target: 1 }, 0, 0).unwrap();
    assert!(!workspace.snapshot().tabs[1].dirty);
}

#[test]
fn opening_focuses_once_and_never_queues_or_starts_playback() {
    let mut w = loaded();
    assert_eq!(w.snapshot().tabs[0].key, QUEUE_TAB);
    assert_eq!(
        w.apply(C::Open {
            key: "album".into(),
            scope: "test".into(),
            playlist_id: 4,
            name: "album".into(),
            readonly: false
        })
        .unwrap(),
        Effect::None
    );
    assert_eq!(w.snapshot().tabs.len(), 2);
    w.apply(C::Focus {
        key: QUEUE_TAB.into(),
    })
    .unwrap();
    w.apply(C::Close {
        key: QUEUE_TAB.into(),
    })
    .unwrap();
    assert_eq!(w.snapshot().tabs.len(), 2);
    assert_eq!(w.snapshot().active, QUEUE_TAB);
}

#[test]
fn duplicate_edits_undo_redo_and_queue_copies_preserve_identity() {
    let mut w = loaded();
    w.apply(C::Select {
        indices: vec![2, 2, 999],
    })
    .unwrap();
    w.apply(C::Nudge { delta: -1 }).unwrap();
    assert_eq!(w.snapshot().selected, vec![1]);
    assert!(w.snapshot().tabs[1].dirty);
    let copied = w
        .apply(C::Queue {
            action: QueueAction::PlayNext,
        })
        .unwrap();
    assert_eq!(
        copied,
        Effect::Queue {
            mode: QueueAction::PlayNext,
            scope: "test".into(),
            entries: vec![json!({"path":"same"})]
        }
    );
    w.apply(C::Remove).unwrap();
    assert_eq!(w.snapshot().entries.len(), 2);
    w.apply(C::Undo).unwrap();
    assert_eq!(w.snapshot().selected, vec![1]);
    w.apply(C::Undo).unwrap();
    assert_eq!(w.snapshot().selected, vec![2]);
    assert!(!w.snapshot().tabs[1].dirty);
    w.apply(C::Redo).unwrap();
    assert_eq!(w.snapshot().selected, vec![1]);
    assert_eq!(
        copied,
        Effect::Queue {
            mode: QueueAction::PlayNext,
            scope: "test".into(),
            entries: vec![json!({"path":"same"})]
        }
    );
}

#[test]
fn dirty_close_and_failed_save_preserve_the_draft() {
    let mut w = loaded();
    w.apply(C::Append {
        entries: vec![json!({"path":"added"})],
    })
    .unwrap();
    w.apply(C::Close {
        key: "album".into(),
    })
    .unwrap();
    assert_eq!(w.snapshot().pending_close.as_deref(), Some("album"));
    w.apply(C::ResolveClose {
        choice: CloseChoice::Cancel,
    })
    .unwrap();
    assert_eq!(w.snapshot().entries.len(), 4);
    w.apply(C::Close {
        key: "album".into(),
    })
    .unwrap();
    let Effect::Save {
        key,
        revision,
        expected_entries,
        ..
    } = w
        .apply(C::ResolveClose {
            choice: CloseChoice::Save,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(expected_entries.len(), 3);
    w.apply(C::SaveFailed {
        key: key.clone(),
        revision,
        error: "offline".into(),
    })
    .unwrap();
    assert!(w.snapshot().tabs[1].dirty);
    assert_eq!(w.snapshot().error.as_deref(), Some("offline"));
    w.apply(C::Close { key }).unwrap();
    w.apply(C::ResolveClose {
        choice: CloseChoice::Discard,
    })
    .unwrap();
    assert_eq!(w.snapshot().tabs.len(), 1);
}

#[test]
fn edits_during_save_stay_dirty_and_stale_replies_do_not_close_reopened_tabs() {
    let mut w = loaded();
    w.apply(C::Append {
        entries: vec![json!(1)],
    })
    .unwrap();
    w.apply(C::Close {
        key: "album".into(),
    })
    .unwrap();
    let Effect::Save { key, revision, .. } = w
        .apply(C::ResolveClose {
            choice: CloseChoice::Save,
        })
        .unwrap()
    else {
        panic!()
    };
    w.apply(C::Append {
        entries: vec![json!(2)],
    })
    .unwrap();
    w.apply(C::Saved {
        key: key.clone(),
        revision,
    })
    .unwrap();
    assert_eq!(w.snapshot().tabs.len(), 2);
    assert!(w.snapshot().tabs[1].dirty);
    w.apply(C::Close { key: key.clone() }).unwrap();
    w.apply(C::ResolveClose {
        choice: CloseChoice::Discard,
    })
    .unwrap();
    let generation = open(&mut w, &key, 4);
    w.apply(C::Loaded {
        key: key.clone(),
        generation,
        entries: vec![json!(3)],
    })
    .unwrap();
    let Effect::Save {
        revision: new_revision,
        ..
    } = w.apply(C::Save).unwrap()
    else {
        panic!()
    };
    assert_ne!(new_revision, revision);
    w.apply(C::Saved {
        key: key.clone(),
        revision,
    })
    .unwrap();
    assert!(w.snapshot().tabs[1].saving);
    w.apply(C::Saved {
        key,
        revision: new_revision,
    })
    .unwrap();
    assert!(!w.snapshot().tabs[1].saving);
}

#[test]
fn stale_loads_and_restarts_do_not_erase_drafts() {
    let mut w = Workspace::default();
    let old = open(&mut w, "album", 4);
    w.apply(C::Close {
        key: "album".into(),
    })
    .unwrap();
    let current = open(&mut w, "album", 4);
    w.apply(C::Loaded {
        key: "album".into(),
        generation: old,
        entries: vec![json!(1)],
    })
    .unwrap();
    assert!(w.snapshot().entries.is_empty());
    w.apply(C::Loaded {
        key: "album".into(),
        generation: current,
        entries: vec![json!(2)],
    })
    .unwrap();
    w.apply(C::Append {
        entries: vec![json!(3)],
    })
    .unwrap();
    let mut restored = Workspace::restore(serde_json::to_value(&w).unwrap()).unwrap();
    assert_eq!(restored.snapshot().entries, vec![json!(2), json!(3)]);
    assert!(restored.snapshot().tabs[1].dirty);
    restored.apply(C::Undo).unwrap();
    assert!(!restored.snapshot().tabs[1].dirty);
}

#[test]
fn favorites_is_readonly_and_queue_actions_share_transport_policy() {
    let mut w = Workspace::default();
    let generation = open(&mut w, "favorites", 0);
    w.apply(C::Loaded {
        key: "favorites".into(),
        generation,
        entries: vec![json!(1)],
    })
    .unwrap();
    assert!(
        w.apply(C::Append {
            entries: vec![json!(2)]
        })
        .is_err()
    );
    assert!(matches!(
        w.apply(C::Queue {
            action: QueueAction::AddToQueue
        })
        .unwrap(),
        Effect::Queue { .. }
    ));
    let mut p = PlaybackOrder::new(ShuffleMode::Off, RepeatMode::Off, 1);
    p.toggle_queue(&[1]);
    assert_eq!(p.apply_queue_action(QueueAction::AddToQueue, 4, 2), None);
    assert_eq!(p.queued_indices(), &[1]);
    p.apply_queue_action(QueueAction::PlayNext, 4, 2);
    assert_eq!(p.queued_indices(), &[4, 5, 1]);
    assert_eq!(
        p.apply_queue_action(QueueAction::PlayNow, 6, 1),
        Some(PlaybackDecision::Play(6))
    );
}

#[test]
fn language_bridge_runs_the_same_workspace_without_changing_transport() {
    use kog_playback_policy::bridge::dispatch_json;
    let mut state = None::<String>;
    let mut send = |command: serde_json::Value| {
        let result = dispatch_json(&json!({"state":state,"command":command}).to_string()).unwrap();
        let reply: serde_json::Value = serde_json::from_str(&result).unwrap();
        state = reply["state"].as_str().map(str::to_owned);
        reply
    };
    send(json!({"op":"init","seed":7,"shuffle":"off","repeat":"all"}));
    send(json!({"op":"sync","current":0,"tracks":[{"id":"playing","album":"A"}]}));
    let reply = send(
        json!({"op":"workspace","command":{"op":"open","key":"server:5","scope":"server","playlist_id":5,"name":"Album"}}),
    );
    assert!(reply["decision"].is_null());
    let generation = reply["workspace_effect"]["generation"].as_u64().unwrap();
    send(
        json!({"op":"workspace","command":{"op":"loaded","key":"server:5","generation":generation,"entries":[{"path":"one"},{"path":"two"}]}}),
    );
    send(json!({"op":"workspace","command":{"op":"select","indices":[1]}}));
    let reply = send(json!({"op":"workspace","command":{"op":"remove"}}));
    assert_eq!(reply["repeat"], "all");
    assert!(reply["decision"].is_null());
    assert_eq!(reply["workspace"]["tabs"][1]["dirty"], true);
    let reply = send(json!({"op":"workspace","command":{"op":"queue","action":"play_next"}}));
    assert!(reply["decision"].is_null());
    assert_eq!(
        reply["workspace_effect"],
        json!({"action":"queue","mode":"play_next","scope":"server","entries":[{"path":"one"}]})
    );
    let reply = send(json!({"op":"workspace","command":{"op":"save"}}));
    assert_eq!(
        reply["workspace_effect"]["expected_entries"],
        json!([{"path":"one"},{"path":"two"}])
    );
}
