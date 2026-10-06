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

fn play_tab(s: &mut Session<Value>, selected: Vec<usize>) {
    workspace_action(s, workspace::Command::Select { indices: selected });
    let effects = workspace_action(
        s,
        workspace::Command::Queue {
            action: workspace::QueueAction::PlayNow,
        },
    );
    // Each port resolves entries asynchronously. Complete in reverse order to
    // catch a prefix/suffix race when starting in the middle of a playlist.
    for effect in effects.into_iter().rev() {
        if let Effect::Expand { token, entries, .. } = effect {
            s.dispatch(Command::Complete {
                token,
                result: IoResult::Expanded { tracks: entries },
            });
        }
    }
}

fn tab(s: &mut Session<Value>, titles: &[&str]) {
    workspace_action(
        s,
        workspace::Command::Open {
            key: "album".into(),
            scope: "local".into(),
            playlist_id: 7,
            name: "Album".into(),
            readonly: false,
        },
    );
    workspace_action(
        s,
        workspace::Command::Loaded {
            key: "album".into(),
            generation: 1,
            entries: titles.iter().map(|t| row(t)).collect(),
        },
    );
}

#[test]
fn next_and_previous_follow_the_playing_tab_not_the_main_queue_or_viewed_tab() {
    let mut s = session("tab-playback");
    append(&mut s, &["unrelated", "main queue"]);
    s.dispatch(Command::ToggleQueued { indices: vec![0] });
    s.dispatch(Command::Repeat {
        mode: RepeatMode::All,
    });
    tab(&mut s, &["first", "middle", "last"]);
    play_tab(&mut s, vec![1]);
    assert_eq!(s.queue()[s.current().unwrap()]["title"], "middle");
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    assert_eq!(
        s.queue()[s.current().unwrap()]["title"],
        "last",
        "Next must stay in the playlist that started playback"
    );
    workspace_action(
        &mut s,
        workspace::Command::Focus {
            key: "queue".into(),
        },
    );
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    assert_eq!(
        s.queue()[s.current().unwrap()]["title"],
        "first",
        "Repeat All must wrap within the playing playlist after browsing another tab"
    );
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Previous,
    });
    assert_eq!(s.queue()[s.current().unwrap()]["title"], "last");
    assert_eq!(
        s.queue()[0]["title"],
        "unrelated",
        "Starting a tab must retain the main queue"
    );
    assert_eq!(s.workspace_model().snapshot().active, "queue");
}

#[test]
fn tab_end_repeat_restore_and_queue_activation_use_the_correct_playback_scope() {
    let mut s = session("tab-boundaries");
    append(&mut s, &["main one", "main two"]);
    tab(&mut s, &["first", "last"]);
    play_tab(&mut s, vec![0]);
    let token = s.snapshot().output_token.unwrap().clone();
    s.dispatch(Command::Output {
        token,
        event: OutputEvent::Ended,
    });
    assert_eq!(s.queue()[s.current().unwrap()]["title"], "last");
    let token = s.snapshot().output_token.unwrap().clone();
    s.dispatch(Command::Output {
        token,
        event: OutputEvent::Ended,
    });
    assert_eq!(s.snapshot().transport, Transport::Stopped);
    assert_eq!(s.queue()[s.current().unwrap()]["title"], "last");

    // Reordering the underlying queue and restoring the player retain the
    // original playing playlist's row identities and traversal order.
    s.dispatch(Command::Reorder {
        indices: vec![3, 1, 2, 0],
    });
    let saved = s.checkpoint();
    s.restore(saved, |v| Ok(v.clone())).unwrap();
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Previous,
    });
    assert_eq!(s.queue()[s.current().unwrap()]["title"], "first");
    s.dispatch(Command::Repeat {
        mode: RepeatMode::One,
    });
    let token = s.snapshot().output_token.unwrap().clone();
    s.dispatch(Command::Output {
        token,
        event: OutputEvent::Ended,
    });
    assert_eq!(s.queue()[s.current().unwrap()]["title"], "first");
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    assert_eq!(s.queue()[s.current().unwrap()]["title"], "last");

    // Explicit playback from the main queue returns to its normal order.
    s.dispatch(Command::Repeat {
        mode: RepeatMode::Off,
    });
    s.dispatch(Command::Play { index: 1 });
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    assert_eq!(s.current(), Some(2));
}

#[test]
fn tab_shuffle_and_late_album_metadata_never_include_other_queue_rows() {
    for mode in [ShuffleMode::All, ShuffleMode::Albums] {
        let mut s = session("tab-shuffle");
        append(&mut s, &["main one", "main two"]);
        tab(&mut s, &["first", "second", "third"]);
        play_tab(&mut s, vec![0]);
        s.dispatch(Command::Repeat {
            mode: RepeatMode::All,
        });
        s.dispatch(Command::Shuffle { mode });
        let rows = s.queue().iter().map(Item::metadata).collect();
        s.dispatch(Command::Metadata { rows });
        for _ in 0..15 {
            s.dispatch(Command::Navigate {
                event: NavigationEvent::Next,
            });
            assert!(s.current().unwrap() >= 2);
        }
        // Removing the playing playlist cannot fall back to the other rows.
        s.dispatch(Command::Remove {
            indices: vec![2, 3, 4],
        });
        let effects = s.dispatch(Command::Navigate {
            event: NavigationEvent::Next,
        });
        assert!(effects.iter().all(|e| !matches!(e, Effect::Play { .. })));
        assert_eq!(s.snapshot().transport, Transport::Stopped);
    }
    let mut s = session("one-song-tab");
    append(&mut s, &["outside"]);
    tab(&mut s, &["only"]);
    play_tab(&mut s, vec![0]);
    s.dispatch(Command::Repeat {
        mode: RepeatMode::All,
    });
    s.dispatch(Command::Shuffle {
        mode: ShuffleMode::All,
    });
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    assert_eq!(s.current(), Some(1));
}

#[test]
fn tab_start_accounts_for_expanded_prefix_and_rejects_superseded_completions() {
    for cancel in [false, true] {
        let mut s = session("tab-expansion");
        append(&mut s, &["outside"]);
        tab(&mut s, &["multi-song file", "clicked", "following"]);
        workspace_action(&mut s, workspace::Command::Select { indices: vec![1] });
        let effects = workspace_action(
            &mut s,
            workspace::Command::Queue {
                action: workspace::QueueAction::PlayNow,
            },
        );
        let jobs: Vec<_> = effects
            .into_iter()
            .filter_map(|e| match e {
                Effect::Expand { token, entries, .. } => Some((token, entries)),
                _ => None,
            })
            .collect();
        assert_eq!(jobs.len(), 2);
        s.dispatch(Command::Complete {
            token: jobs[1].0.clone(),
            result: IoResult::Expanded {
                tracks: jobs[1].1.clone(),
            },
        });
        assert_eq!(
            s.queue(),
            &[row("outside")],
            "A partial load cannot publish a partial playlist"
        );
        if cancel {
            s.dispatch(Command::Stop);
        }
        s.dispatch(Command::Complete {
            token: jobs[0].0.clone(),
            result: IoResult::Expanded {
                tracks: vec![row("subsong 1"), row("subsong 2")],
            },
        });
        if cancel {
            assert_eq!(s.queue(), &[row("outside")]);
            assert_eq!(s.snapshot().transport, Transport::Stopped);
        } else {
            assert_eq!(s.current(), Some(3));
            assert_eq!(s.queue()[3]["title"], "clicked");
            s.dispatch(Command::Navigate {
                event: NavigationEvent::Previous,
            });
            assert_eq!(s.queue()[s.current().unwrap()]["title"], "subsong 2");
        }
    }
}

#[test]
fn duplicate_occurrences_and_explicit_multi_selection_have_bounded_navigation() {
    let mut s = session("tab-duplicates");
    append(&mut s, &["outside"]);
    tab(&mut s, &["same", "middle", "same", "last"]);
    play_tab(&mut s, vec![2]);
    assert_eq!(
        s.current(),
        Some(3),
        "Start at the selected occurrence, not the first matching path"
    );
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    assert_eq!(s.queue()[s.current().unwrap()]["title"], "last");
    play_tab(&mut s, vec![1, 3]);
    assert_eq!(s.queue()[s.current().unwrap()]["title"], "middle");
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    assert_eq!(s.queue()[s.current().unwrap()]["title"], "last");
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    assert_eq!(s.snapshot().transport, Transport::Stopped);
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

fn activate_tab(s: &mut Session<Value>, index: usize) -> Vec<Effect> {
    let mut effects = workspace_action(s, workspace::Command::Activate { index });
    for effect in effects.clone().into_iter().rev() {
        if let Effect::Expand { token, entries, .. } = effect {
            effects.extend(s.dispatch(Command::Complete {
                token,
                result: IoResult::Expanded { tracks: entries },
            }));
        }
    }
    effects
}

#[test]
fn playlist_activation_toggles_the_exact_duplicate_row_without_restarting_or_appending() {
    let mut s = session("activate-tab");
    append(&mut s, &["unrelated"]);
    tab(&mut s, &["same", "middle", "same"]);
    let token = play_token(activate_tab(&mut s, 2));
    s.dispatch(Command::Output {
        token: token.clone(),
        event: OutputEvent::Started,
    });
    s.dispatch(Command::Seek { seconds: 12.0 });
    assert_eq!(s.current(), Some(3));
    assert_eq!(s.workspace().current, Some(2));
    let queued = s.snapshot().row_ids.to_vec();
    let effects = activate_tab(&mut s, 2);
    assert!(effects.iter().any(|e| matches!(e, Effect::Pause)));
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::Play { .. } | Effect::Expand { .. }))
    );
    assert_eq!(s.snapshot().position, 12.0);
    assert_eq!(s.snapshot().output_token, Some(&token));
    assert!(
        activate_tab(&mut s, 2)
            .iter()
            .any(|e| matches!(e, Effect::Resume))
    );
    assert_eq!(s.snapshot().row_ids, queued);
    assert_eq!(s.snapshot().position, 12.0);
    let effects = activate_tab(&mut s, 0);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Play { index: 1, .. }))
    );
    assert_eq!(s.workspace().current, Some(0));
    assert_eq!(s.snapshot().row_ids, queued);
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    assert_eq!(s.current(), Some(2));
    assert_eq!(s.workspace().current, Some(1));
    assert!(
        activate_tab(&mut s, 1)
            .iter()
            .any(|e| matches!(e, Effect::Pause))
    );
}

#[test]
fn activation_survives_reordering_and_checkpoint_restore_without_mistaking_same_named_songs() {
    let mut s = session("activate-reorder");
    tab(&mut s, &["first", "middle", "last"]);
    activate_tab(&mut s, 1);
    workspace_action(&mut s, workspace::Command::Move { target: 3 });
    assert_eq!(s.workspace().current, Some(2));
    assert!(
        activate_tab(&mut s, 2)
            .iter()
            .any(|e| matches!(e, Effect::Pause))
    );
    assert_eq!(s.queue().len(), 3);
    let saved = s.checkpoint();
    s.restore(saved, |v| Ok(v.clone())).unwrap();
    assert_eq!(s.workspace().current, Some(2));
    assert!(
        activate_tab(&mut s, 2)
            .iter()
            .any(|e| matches!(e, Effect::Play { index: 1, .. }))
    );
    assert_eq!(s.queue().len(), 3);
    s.dispatch(Command::Reorder {
        indices: vec![2, 0, 1],
    });
    assert_eq!(s.workspace().current, Some(2));
    assert!(
        activate_tab(&mut s, 2)
            .iter()
            .any(|e| matches!(e, Effect::Pause))
    );
}

#[test]
fn queue_pause_resume_preserves_the_playing_playlist_navigation_scope() {
    let mut s = session("activate-queue");
    append(&mut s, &["unrelated"]);
    tab(&mut s, &["first", "last"]);
    activate_tab(&mut s, 1);
    workspace_action(
        &mut s,
        workspace::Command::Focus {
            key: "queue".into(),
        },
    );
    assert!(
        s.dispatch(Command::Activate { index: 2 })
            .iter()
            .any(|e| matches!(e, Effect::Pause))
    );
    assert!(
        s.dispatch(Command::Activate { index: 2 })
            .iter()
            .any(|e| matches!(e, Effect::Resume))
    );
    s.dispatch(Command::Repeat {
        mode: RepeatMode::All,
    });
    s.dispatch(Command::Navigate {
        event: NavigationEvent::Next,
    });
    assert_eq!(
        s.current(),
        Some(1),
        "repeat must stay inside the playlist after pausing from the queue"
    );
}

#[test]
fn activation_recognizes_explicit_selections_and_unique_tracks_started_in_the_queue() {
    let mut s = session("activate-selection");
    tab(&mut s, &["same", "middle", "same"]);
    play_tab(&mut s, vec![0, 2]);
    assert_eq!(s.workspace().current, Some(0));
    assert!(
        activate_tab(&mut s, 0)
            .iter()
            .any(|e| matches!(e, Effect::Pause))
    );
    assert_eq!(s.queue().len(), 2);

    let mut s = session("activate-queue-source");
    append(&mut s, &["middle"]);
    s.dispatch(Command::Play { index: 0 });
    tab(&mut s, &["same", "middle", "same"]);
    assert_eq!(s.workspace().current, Some(1));
    assert!(
        activate_tab(&mut s, 1)
            .iter()
            .any(|e| matches!(e, Effect::Pause))
    );
    assert_eq!(s.queue().len(), 1);
}

#[test]
fn workspace_queue_activation_selects_and_plays_the_requested_row_atomically() {
    let mut s = session("activate-queue-workspace");
    append(&mut s, &["first", "second"]);
    workspace_action(&mut s, workspace::Command::Activate { index: 1 });
    assert_eq!(s.current(), Some(1));
    assert_eq!(s.workspace().selected, vec![1]);
    assert_eq!(s.snapshot().selection.anchor, Some(1));
    let effects = workspace_action(&mut s, workspace::Command::Activate { index: 1 });
    assert!(effects.iter().any(|e| matches!(e, Effect::Pause)));
}
