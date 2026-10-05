//! Terminal I/O ports and view projection for the shared application session.
use super::*;
use serde_json::{Value, json};

impl SessionItem for Track {
    fn entry(&self) -> Value {
        let mut value = kog_server::api::entry_json(self.entry.clone());
        value["name"] = Value::String(self.name.clone());
        value
    }
    fn metadata(&self) -> SortRow {
        kog_audio::playback_order::session::metadata_from_json(&self.entry())
    }
}
pub(super) fn decode_track(value: &Value) -> Result<Track, String> {
    let entry = kog_server::api::stored_entries_from_json(std::slice::from_ref(value))?
        .into_iter()
        .next()
        .ok_or("Missing queue entry")?;
    let mut track = track_from_entry(entry);
    if let Some(name) = value["name"].as_str() {
        track.name = name.to_owned();
    }
    Ok(track)
}
fn prepare_entries(
    entries: Vec<Value>,
    remote: Option<RemoteSettings>,
    library: Arc<Library>,
    decoders: DecoderRegistry,
) -> Result<Vec<Track>, String> {
    let mut tracks = Vec::new();
    for value in entries {
        if let Some(path) = value["scan_path"].as_str() {
            let query = value["scan_query"].as_str().unwrap_or_default();
            if let Some(remote) = &remote {
                tracks.extend(
                    remote
                        .collect_folder(Path::new(path), query)?
                        .into_iter()
                        .map(|file| remote_track(remote, file))
                        .collect::<Result<Vec<_>, _>>()?,
                );
            } else {
                let settings = AppSettings::load();
                let filter = kog_audio::library_policy::TreeFilter::new(
                    query,
                    PathBuf::from(value["scan_root"].as_str().unwrap_or_default()),
                );
                tracks.extend(
                    kog_server::api::collect_local_folder_filtered(
                        &library,
                        &decoders,
                        Path::new(path),
                        true,
                        settings.read_cue_sheets_in_folders,
                        settings.read_playlists_in_folders,
                        &filter,
                    )?
                    .into_iter()
                    .map(|(name, entry)| Track { name, entry }),
                );
            }
        } else {
            let track = decode_track(&value)?;
            if let Some(remote) = &remote {
                if let Some(file) =
                    RemoteFile::from_stream_url(track.name.clone(), &track.entry.path)
                {
                    tracks.extend(
                        remote
                            .expand_files(&[file])?
                            .into_iter()
                            .map(|file| remote_track(remote, file))
                            .collect::<Result<Vec<_>, _>>()?,
                    );
                    continue;
                }
            }
            tracks.extend(expand_track(&decoders, library.root().as_deref(), track));
        }
    }
    Ok(tracks)
}
impl Ui {
    pub(super) fn remote_scope(&self) -> String {
        self.remote_connection
            .as_ref()
            .map(|s| format!("server:{}", s.server_url.trim_end_matches('/')))
            .unwrap_or_else(|| "unavailable".into())
    }
    pub(super) fn queue_session(&mut self, command: SessionCommand<Track>) {
        let update = matches!(&command, SessionCommand::UpdateItem { .. });
        let effects = self.session.dispatch(command);
        if update
            || effects
                .iter()
                .any(|e| matches!(e, SessionEffect::QueueChanged { .. }))
        {
            self.tracks = self.session.queue().to_vec();
            self.auto_fit_cache = None;
        }
        self.session_effects.extend(effects);
    }
    pub(super) fn session_command(&mut self, command: SessionCommand<Track>) {
        let mut scopes = vec!["local".into()];
        if self.remote_connection.is_some() {
            scopes.push(self.remote_scope());
        }
        // Connection changes invalidate pending results from the previous source.
        let _ = self.session.dispatch(SessionCommand::Scopes { scopes });
        self.queue_session(command);
        self.flush_session_effects();
    }
    pub(super) fn apply_session_view(&mut self) {
        let view = self.session.snapshot();
        self.playing = view.current;
        self.volume = view.volume as f32;
        self.repeat_mode = view.repeat;
        self.shuffle_mode = view.shuffle;
        self.radio_enabled = view.radio_enabled;
        self.playlist_query = view.filter.to_owned();
        self.selected_tracks = view.selection.indices.iter().copied().collect();
        self.selection_anchor = view.selection.anchor;
        if let Some(error) = view.error {
            self.status = error.to_owned();
        }
        self.selected[2] = self.selected[2].min(self.tracks.len().saturating_sub(1));
        self.sync_workspace_view();
    }
    pub(super) fn flush_session_effects(&mut self) {
        self.apply_session_view();
        while let Some(effect) = self.session_effects.pop_front() {
            match effect {
                SessionEffect::Persist { .. } => self.session_dirty = true,
                SessionEffect::QueueChanged { old_to_new } => {
                    self.selected[2] = old_to_new
                        .get(self.selected[2])
                        .copied()
                        .flatten()
                        .unwrap_or_default();
                    self.auto_fit_cache = None;
                }
                SessionEffect::Play {
                    token,
                    index,
                    seconds,
                    playing,
                } => {
                    self.apply_session_view();
                    let result = self
                        .tracks
                        .get(index)
                        .ok_or_else(|| "Missing playback row".to_owned())
                        .and_then(|track| PlaylistEntry::try_from(&track.entry))
                        .and_then(|entry| {
                            kog_audio::streaming::resolve_entry(
                                &entry,
                                &self.decoders,
                                &kog_server::service::scratch_root().join("tui"),
                            )
                        })
                        .and_then(|source| self.player.play_source(&source).map(|_| ()));
                    match result {
                        Ok(()) => {
                            if seconds > 0.0 {
                                if let Err(error) =
                                    self.player.seek(Duration::from_secs_f64(seconds))
                                {
                                    self.status = error;
                                }
                            }
                            if !playing {
                                self.player.play_pause();
                            }
                            self.queue_session(SessionCommand::Output {
                                token,
                                event: OutputEvent::Started,
                            });
                            self.status = "Playing".into();
                        }
                        Err(error) => self.queue_session(SessionCommand::Output {
                            token,
                            event: OutputEvent::Failed { error },
                        }),
                    }
                    self.auto_fit_cache = None;
                }
                SessionEffect::Stop => {
                    self.player.stop();
                    self.auto_fit_cache = None;
                }
                SessionEffect::Pause => {
                    if self.player.state() == PlaybackState::Playing {
                        self.player.play_pause();
                    }
                }
                SessionEffect::Resume => {
                    if self.player.state() == PlaybackState::Paused {
                        self.player.play_pause();
                    }
                }
                SessionEffect::Seek { seconds } => {
                    if let Err(error) = self.player.seek(Duration::from_secs_f64(seconds)) {
                        self.status = error;
                    }
                }
                SessionEffect::Volume { value } => self.player.set_volume(value as f32),
                SessionEffect::Load {
                    token, playlist_id, ..
                } => {
                    let result = if playlist_id == 0 {
                        self.library.db().starred_entries()
                    } else {
                        self.library.db().playlist_entries(playlist_id)
                    };
                    let result = match result {
                        Ok(rows) => IoResult::Loaded {
                            entries: rows.into_iter().map(kog_server::api::entry_json).collect(),
                        },
                        Err(error) => IoResult::Failed { error },
                    };
                    self.queue_session(SessionCommand::Complete { token, result });
                }
                SessionEffect::Save {
                    token,
                    playlist_id,
                    entries,
                    expected_entries,
                    ..
                } => {
                    let result =
                        kog_server::api::stored_entries_from_json(&entries).and_then(|entries| {
                            let expected =
                                kog_server::api::stored_entries_from_json(&expected_entries)?;
                            self.library.db().replace_entries_checked(
                                playlist_id,
                                &entries,
                                Some(&expected),
                            )
                        });
                    let result = match result {
                        Ok(()) => {
                            self.reload_lists();
                            self.status = "Playlist saved".into();
                            IoResult::Saved
                        }
                        Err(error) => IoResult::Failed { error },
                    };
                    self.queue_session(SessionCommand::Complete { token, result });
                }
                SessionEffect::Expand {
                    token,
                    scope,
                    entries,
                } => self.prepare_session(token, scope, entries),
                SessionEffect::Collect {
                    token,
                    scope,
                    path,
                    query,
                    root,
                } => self.prepare_session(
                    token,
                    scope,
                    vec![json!({"scan_path":path,"scan_query":query,"scan_root":root})],
                ),
                effect @ SessionEffect::Radio { .. } => {
                    let token = match &effect {
                        SessionEffect::Radio { token, .. } => token.clone(),
                        _ => unreachable!(),
                    };
                    if let Err(error) = self.radio.request(effect) {
                        self.queue_session(SessionCommand::Complete {
                            token,
                            result: IoResult::Failed { error },
                        });
                    }
                }
            }
        }
        self.apply_session_view();
    }
    fn prepare_session(&mut self, token: Token, scope: String, entries: Vec<Value>) {
        let remote = if scope == "local" {
            None
        } else {
            self.remote_connection.clone()
        };
        if scope != "local" && remote.is_none() {
            self.queue_session(SessionCommand::Complete {
                token,
                result: IoResult::Failed {
                    error: "Connect to the original source first".into(),
                },
            });
            return;
        }
        let library = self.library.clone();
        let decoders = self
            .decoders
            .background_worker(self.decoder_settings.clone());
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = match prepare_entries(entries, remote, library, decoders) {
                Ok(tracks) => IoResult::Expanded { tracks },
                Err(error) => IoResult::Failed { error },
            };
            let _ = sender.send(result);
        });
        self.session_jobs.push((token, receiver));
        self.status = "Preparing playlist tracks…".into();
    }
    pub(super) fn poll_session_ports(&mut self) {
        let mut ready = self.radio.poll();
        self.session_jobs
            .retain(|(token, receiver)| match receiver.try_recv() {
                Ok(result) => {
                    ready.push((token.clone(), result));
                    false
                }
                Err(mpsc::TryRecvError::Empty) => true,
                Err(mpsc::TryRecvError::Disconnected) => {
                    ready.push((
                        token.clone(),
                        IoResult::Failed {
                            error: "Track preparation stopped".into(),
                        },
                    ));
                    false
                }
            });
        for (token, result) in ready {
            self.session_command(SessionCommand::Complete { token, result });
        }
        if !self.session_effects.is_empty() {
            self.flush_session_effects();
        }
    }
    pub(super) fn report_session_progress(&mut self) {
        if let Some(token) = self.session.snapshot().output_token.cloned() {
            let seconds = self.player.position().as_secs_f64();
            let duration = self
                .playing
                .and_then(|i| self.tracks.get(i))
                .and_then(|t| self.metadata_for(t))
                .and_then(|m| m.duration)
                .map(|d| d.as_secs_f64())
                .unwrap_or_default();
            let _ = self.session.dispatch(SessionCommand::Output {
                token,
                event: OutputEvent::Progress { seconds, duration },
            });
        }
    }
    pub(super) fn seek_session(&mut self, seconds: f64) -> Result<(), String> {
        self.session_command(SessionCommand::Seek { seconds });
        self.session
            .snapshot()
            .error
            .map(|s| Err(s.to_owned()))
            .unwrap_or(Ok(()))
    }
}
