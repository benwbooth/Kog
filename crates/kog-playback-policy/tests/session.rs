use kog_playback_policy::{NavigationEvent, RepeatMode, ShuffleMode, session::*, workspace};
use serde_json::{Value, json};

#[test]
fn edit_commands_target_the_active_queue_and_preserve_row_identity() {
    use kog_playback_policy::selection::Command as Select;
    let mut s = session("edit");
    s.dispatch(Command::Append {
        tracks: vec![
            json!({"path":"a"}),
            json!({"path":"b"}),
            json!({"path":"a"}),
        ],
        action: workspace::QueueAction::AddToQueue,
    });
    let ids = s.snapshot().row_ids.to_vec();
    s.dispatch(Command::Play { index: 2 });
    workspace_action(
        &mut s,
        workspace::Command::Selection {
            command: Select::Set {
                indices: vec![2],
                anchor: Some(2),
            },
        },
    );
    assert!(s.workspace().actions.move_up);
    assert!(!s.workspace().actions.move_down);
    let effects = workspace_action(&mut s, workspace::Command::Nudge { delta: -1 });
    assert_eq!(s.snapshot().row_ids, &[ids[0], ids[2], ids[1]]);
    assert_eq!(s.snapshot().current, Some(1));
    assert_eq!(s.workspace().selected, vec![1]);
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::Play { .. } | Effect::Stop))
    );
    s.dispatch(Command::Filter { query: "b".into() });
    workspace_action(
        &mut s,
        workspace::Command::Selection {
            command: Select::All,
        },
    );
    assert_eq!(s.workspace().selected, vec![2]);
    workspace_action(&mut s, workspace::Command::Remove);
    assert_eq!(s.queue().len(), 2);
    assert_eq!(s.snapshot().current, Some(1));
    assert!(!s.workspace().actions.select_all);
    workspace_action(
        &mut s,
        workspace::Command::Selection {
            command: Select::All,
        },
    );
    assert!(
        s.workspace().selected.is_empty(),
        "Empty search results must not select hidden rows"
    );
    assert!(s.workspace().actions.clear);
    workspace_action(&mut s, workspace::Command::Clear);
    assert!(s.queue().is_empty());
    assert!(!s.workspace().actions.clear);
    assert!(!s.workspace().actions.remove);
}
fn session(id: &str) -> Session<Value> {
    Session::new(id, 17, ShuffleMode::Off, RepeatMode::Off)
}
fn row(title: &str) -> Value {
    json!({"kind":"local","path":format!("/{title}.wav"),"name":title,"title":title,"album":"Album"})
}
fn append(s: &mut Session<Value>, titles: &[&str]) {
    s.dispatch(Command::Append {
        tracks: titles.iter().map(|t| row(t)).collect(),
        action: workspace::QueueAction::AddToQueue,
    });
}
fn expand(s: &mut Session<Value>, action: workspace::QueueAction) -> Token {
    s.dispatch(Command::Expand {
        scope: "local".into(),
        entries: vec![row("one")],
        action,
    })
    .into_iter()
    .find_map(|e| match e {
        Effect::Expand { token, .. } => Some(token),
        _ => None,
    })
    .unwrap()
}
fn play_token(effects: Vec<Effect>) -> Token {
    effects
        .into_iter()
        .find_map(|e| match e {
            Effect::Play { token, .. } => Some(token),
            _ => None,
        })
        .unwrap()
}

#[test]
fn sessions_own_independent_queues_modes_drafts_and_effects() {
    let mut a = session("qt");
    let mut b = session("phone");
    append(&mut a, &["A", "A"]);
    append(&mut b, &["B"]);
    a.dispatch(Command::Shuffle {
        mode: ShuffleMode::All,
    });
    let token = expand(&mut a, workspace::QueueAction::AddToQueue);
    let before = b.checkpoint();
    assert!(
        b.dispatch(Command::Complete {
            token: token.clone(),
            result: IoResult::Expanded {
                tracks: vec![row("wrong")]
            }
        })
        .is_empty()
    );
    assert_eq!(before, b.checkpoint());
    assert_eq!(b.snapshot().shuffle, ShuffleMode::Off);
    a.dispatch(Command::Complete {
        token,
        result: IoResult::Expanded {
            tracks: vec![row("C")],
        },
    });
    assert_eq!(a.queue().len(), 3);
    assert_eq!(b.queue(), [row("B")]);
    assert_ne!(
        a.snapshot().row_ids[0],
        a.snapshot().row_ids[1],
        "duplicate locators retain different row identity"
    );
}

#[test]
fn expansion_completion_is_fifo_and_clear_invalidates_pending_jobs() {
    let mut s = session("web");
    let first = expand(&mut s, workspace::QueueAction::AddToQueue);
    let second = expand(&mut s, workspace::QueueAction::AddToQueue);
    s.dispatch(Command::Complete {
        token: second,
        result: IoResult::Expanded {
            tracks: vec![row("second")],
        },
    });
    assert!(s.queue().is_empty());
    s.dispatch(Command::Complete {
        token: first,
        result: IoResult::Expanded {
            tracks: vec![row("first")],
        },
    });
    assert_eq!(s.queue(), [row("first"), row("second")]);
    let pending = expand(&mut s, workspace::QueueAction::PlayNow);
    s.dispatch(Command::Clear);
    assert!(
        s.dispatch(Command::Complete {
            token: pending,
            result: IoResult::Expanded {
                tracks: vec![row("stale")]
            }
        })
        .is_empty()
    );
    assert!(s.queue().is_empty());
    assert_eq!(s.snapshot().transport, Transport::Stopped);
}

#[test]
fn stop_cancels_pending_autoplay_without_discarding_queue_adds() {
    let mut s = session("ios");
    let pending = expand(&mut s, workspace::QueueAction::PlayNow);
    s.dispatch(Command::Stop);
    let effects = s.dispatch(Command::Complete {
        token: pending,
        result: IoResult::Expanded {
            tracks: vec![row("late")],
        },
    });
    assert_eq!(s.queue().len(), 1);
    assert!(!effects.iter().any(|e| matches!(e, Effect::Play { .. })));
    assert_eq!(s.current(), None);
}

#[test]
fn output_callbacks_follow_identity_and_reject_superseded_plays() {
    let mut s = session("android");
    append(&mut s, &["A", "B", "C"]);
    let old = play_token(s.dispatch(Command::Play { index: 0 }));
    let current = play_token(s.dispatch(Command::Play { index: 2 }));
    s.dispatch(Command::Move {
        indices: vec![2],
        target: 0,
    });
    s.dispatch(Command::Output {
        token: current.clone(),
        event: OutputEvent::Started,
    });
    assert_eq!(s.current(), Some(0));
    assert_eq!(s.queue()[0], row("C"));
    assert_eq!(s.snapshot().transport, Transport::Playing);
    assert!(
        s.dispatch(Command::Output {
            token: old,
            event: OutputEvent::Ended
        })
        .is_empty()
    );
    assert_eq!(s.current(), Some(0));
    let effects = s.dispatch(Command::Remove { indices: vec![0] });
    assert!(effects.iter().any(|e| matches!(e, Effect::Stop)));
    assert_eq!(s.current(), None);
    assert!(
        s.dispatch(Command::Output {
            token: current,
            event: OutputEvent::Started
        })
        .is_empty()
    );
}

#[test]
fn checkpoint_restores_only_its_own_session_and_never_autoplays() {
    let mut s = session("terminal");
    append(&mut s, &["A", "B"]);
    let token = play_token(s.dispatch(Command::Play { index: 1 }));
    s.dispatch(Command::Output {
        token: token.clone(),
        event: OutputEvent::Started,
    });
    let saved = s.checkpoint();
    assert!(
        session("qt")
            .restore(saved.clone(), |v| Ok(v.clone()))
            .is_err()
    );
    s.restore(saved, |v| Ok(v.clone())).unwrap();
    assert_eq!(s.current(), Some(1));
    assert_eq!(s.snapshot().transport, Transport::Stopped);
    s.dispatch(Command::Play { index: 0 });
    assert!(
        s.dispatch(Command::Output {
            token,
            event: OutputEvent::Ended
        })
        .is_empty()
    );
    assert_eq!(s.current(), Some(0));
}

#[test]
fn sort_filter_and_empty_select_all_are_backend_state() {
    let mut s = session("qt");
    append(&mut s, &["Track 10", "Track 2", "Other"]);
    s.dispatch(Command::Sort {
        column: "title".into(),
        descending: false,
        physical: true,
    });
    assert_eq!(s.queue(), [row("Other"), row("Track 2"), row("Track 10")]);
    assert_eq!(s.visible(), [0, 1, 2]);
    s.dispatch(Command::Filter {
        query: "Track".into(),
    });
    s.dispatch(Command::Select {
        command: kog_playback_policy::selection::Command::All,
    });
    assert_eq!(s.selection().indices, [1, 2]);
    s.dispatch(Command::Filter {
        query: "no match".into(),
    });
    s.dispatch(Command::Select {
        command: kog_playback_policy::selection::Command::All,
    });
    assert!(s.selection().indices.is_empty());
    assert_eq!(s.queue().len(), 3);
}

#[test]
fn workspace_effects_are_completed_by_the_session_and_do_not_mutate_queue() {
    let mut s = session("qt");
    append(&mut s, &["queue"]);
    let effects = s.dispatch(Command::Workspace {
        command: workspace::Command::Open {
            key: "local:1".into(),
            scope: "local".into(),
            playlist_id: 1,
            name: "Draft".into(),
            readonly: false,
        },
    });
    let token = effects
        .into_iter()
        .find_map(|e| match e {
            Effect::Load { token, .. } => Some(token),
            _ => None,
        })
        .unwrap();
    s.dispatch(Command::Complete {
        token,
        result: IoResult::Loaded {
            entries: vec![row("draft")],
        },
    });
    s.dispatch(Command::AppendQueueToWorkspace {
        selected_only: false,
    });
    assert_eq!(s.workspace().entries.len(), 2);
    assert_eq!(s.queue().len(), 1);
    s.dispatch(Command::Workspace {
        command: workspace::Command::Close {
            key: "local:1".into(),
        },
    });
    assert!(s.workspace().pending_close.is_some());
    assert_eq!(s.snapshot().transport, Transport::Stopped);
}

#[test]
fn automatic_failures_are_bounded_even_with_repeat_all() {
    let mut s = session("web");
    append(&mut s, &["A", "B", "C"]);
    s.dispatch(Command::Repeat {
        mode: RepeatMode::All,
    });
    let mut effects = s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    let mut attempts = 0;
    while let Some(token) = effects.iter().find_map(|e| match e {
        Effect::Play { token, .. } => Some(token.clone()),
        _ => None,
    }) {
        attempts += 1;
        assert!(attempts <= 3);
        effects = s.dispatch(Command::Output {
            token,
            event: OutputEvent::Failed {
                error: "unreadable".into(),
            },
        });
    }
    assert_eq!(attempts, 3);
    assert_eq!(s.snapshot().transport, Transport::Stopped);
}

#[test]
fn fifo_play_requests_commit_only_the_last_output_without_cancelling_later_adds() {
    let mut s = session("fifo");
    let a = expand(&mut s, workspace::QueueAction::PlayNow);
    let b = expand(&mut s, workspace::QueueAction::PlayNow);
    s.dispatch(Command::Complete {
        token: b,
        result: IoResult::Expanded {
            tracks: vec![row("B")],
        },
    });
    let effects = s.dispatch(Command::Complete {
        token: a,
        result: IoResult::Expanded {
            tracks: vec![row("A")],
        },
    });
    assert_eq!(s.current(), Some(1));
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Play { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::QueueChanged { .. }))
            .count(),
        1
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Play { index: 1, .. }))
    );
}

fn append_to(s: &mut Session<Value>, key: &str, entries: Vec<Value>) -> Vec<Effect> {
    s.dispatch(Command::AppendToTab {
        key: key.into(),
        scope: "local".into(),
        entries,
    })
}
fn complete_append(
    s: &mut Session<Value>,
    effects: Vec<Effect>,
    tracks: Vec<Value>,
) -> Vec<Effect> {
    let token = effects
        .into_iter()
        .find_map(|e| match e {
            Effect::Expand { token, .. } => Some(token),
            _ => None,
        })
        .unwrap();
    s.dispatch(Command::Complete {
        token,
        result: IoResult::Expanded { tracks },
    })
}
fn workspace_action(s: &mut Session<Value>, command: workspace::Command) -> Vec<Effect> {
    s.dispatch(Command::Workspace { command })
}

#[test]
fn append_to_queue_is_one_undo_step_and_preserves_playback_and_duplicates() {
    let mut s = session("qt");
    append(&mut s, &["playing", "original"]);
    let token = play_token(s.dispatch(Command::Play { index: 0 }));
    s.dispatch(Command::Output {
        token: token.clone(),
        event: OutputEvent::Started,
    });
    s.dispatch(Command::Pause);
    s.dispatch(Command::Output {
        token: token.clone(),
        event: OutputEvent::Progress {
            seconds: 17.0,
            duration: 120.0,
        },
    });
    let original_rows = s.snapshot().row_ids.to_vec();
    let tracks = vec![row("original"), row("added"), row("added")];
    let request = append_to(&mut s, "queue", tracks.clone());
    let effects = complete_append(&mut s, request, tracks.clone());
    assert_eq!(&s.queue()[2..], tracks);
    assert!(!effects.iter().any(|e| matches!(
        e,
        Effect::Play { .. } | Effect::Pause | Effect::Resume | Effect::Stop
    )));
    assert_eq!(s.snapshot().transport, Transport::Paused);
    assert_eq!(s.snapshot().position, 17.0);
    assert_eq!(s.snapshot().output_token, Some(&token));
    let effects = workspace_action(&mut s, workspace::Command::Undo);
    assert_eq!(s.snapshot().row_ids, original_rows);
    assert_eq!(s.queue(), [row("playing"), row("original")]);
    assert_eq!(s.snapshot().output_token, Some(&token));
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::Stop | Effect::Play { .. }))
    );
    assert!(s.workspace().actions.redo);
    let redo = workspace_action(&mut s, workspace::Command::Redo);
    complete_append(&mut s, redo, tracks.clone());
    assert_eq!(&s.queue()[2..], tracks);
    assert_eq!(s.snapshot().transport, Transport::Paused);
    assert_eq!(s.snapshot().position, 17.0);
    assert!(s.workspace().actions.undo);
}

#[test]
fn undo_pending_append_ignores_late_completion_without_losing_other_io() {
    let mut s = session("qt");
    let first = append_to(&mut s, "queue", vec![row("cancelled")]);
    let second = expand(&mut s, workspace::QueueAction::AddToQueue);
    s.dispatch(Command::Complete {
        token: second,
        result: IoResult::Expanded {
            tracks: vec![row("keep")],
        },
    });
    assert!(s.queue().is_empty());
    workspace_action(&mut s, workspace::Command::Undo);
    assert_eq!(s.queue(), [row("keep")]);
    assert!(complete_append(&mut s, first, vec![row("late")]).is_empty());
    assert_eq!(s.queue(), [row("keep")]);
}

#[test]
fn append_targets_an_inactive_draft_without_changing_queue_or_active_tab() {
    let mut s = session("qt");
    append(&mut s, &["queue"]);
    for (key, id, readonly) in [("local:1", 1, false), ("local:0", 0, true)] {
        let load = workspace_action(
            &mut s,
            workspace::Command::Open {
                key: key.into(),
                scope: "local".into(),
                playlist_id: id,
                name: key.into(),
                readonly,
            },
        );
        let token = load
            .into_iter()
            .find_map(|e| match e {
                Effect::Load { token, .. } => Some(token),
                _ => None,
            })
            .unwrap();
        s.dispatch(Command::Complete {
            token,
            result: IoResult::Loaded {
                entries: vec![row("existing")],
            },
        });
    }
    workspace_action(
        &mut s,
        workspace::Command::Focus {
            key: "queue".into(),
        },
    );
    let before = s.queue().to_vec();
    let entries = vec![row("A"), row("B"), row("A")];
    let effects = append_to(&mut s, "local:1", entries.clone());
    assert!(!effects.iter().any(|e| matches!(
        e,
        Effect::Expand { .. } | Effect::Play { .. } | Effect::QueueChanged { .. }
    )));
    assert_eq!(s.workspace().active, "queue");
    assert_eq!(s.queue(), before);
    workspace_action(
        &mut s,
        workspace::Command::Focus {
            key: "local:1".into(),
        },
    );
    assert_eq!(&s.workspace().entries[1..], entries);
    workspace_action(&mut s, workspace::Command::Undo);
    assert_eq!(s.workspace().entries, [row("existing")]);
    let unchanged = s.checkpoint();
    append_to(&mut s, "local:0", vec![row("forbidden")]);
    assert_eq!(s.checkpoint(), unchanged, "Favorites cannot receive tracks");
    append_to(&mut s, "missing", vec![row("stale drop")]);
    assert_eq!(
        s.checkpoint(),
        unchanged,
        "A stale drop never redirects to the active tab"
    );
}
