//! Language-neutral command adapter for Swift and Kotlin. This serializes the
//! same policy types used directly by Qt, terminal, and WebAssembly; it does
//! not implement another shuffle, queue, transport, or radio algorithm.

use crate::radio::RadioBuffer;
use crate::sort::{SortRow, SortValue, sorted_indices, sorted_rows};
use crate::{
    NavigationEvent, OrderTrack, PlaybackDecision, PlaybackOrder, RepeatMode, ShuffleMode,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyState {
    order: PlaybackOrder,
    tracks: Vec<OrderTrack>,
    ids: Vec<String>,
    radio: RadioBuffer<Value>,
}

impl Default for PolicyState {
    fn default() -> Self {
        Self {
            order: PlaybackOrder::new(ShuffleMode::Off, RepeatMode::Off, 1),
            tracks: Vec::new(),
            ids: Vec::new(),
            radio: RadioBuffer::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyTrack {
    pub id: String,
    #[serde(flatten)]
    pub order: OrderTrack,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Init {
        seed: u64,
        shuffle: ShuffleMode,
        repeat: RepeatMode,
    },
    Sync {
        tracks: Vec<PolicyTrack>,
        current: Option<usize>,
        #[serde(default)]
        old_to_new: Option<Vec<Option<usize>>>,
    },
    Navigate {
        event: NavigationEvent,
        current: Option<usize>,
    },
    Started {
        previous: Option<usize>,
        index: usize,
    },
    RadioCandidate {
        index: usize,
    },
    CancelNavigation,
    SetShuffle {
        mode: ShuffleMode,
        current: Option<usize>,
    },
    CycleShuffle {
        current: Option<usize>,
    },
    SetRepeat {
        mode: RepeatMode,
    },
    CycleRepeat,
    ToggleQueue {
        indices: Vec<usize>,
    },
    ToggleStopAfter {
        indices: Vec<usize>,
    },
    ClearQueue,
    RadioReset {
        enabled: bool,
        current: Option<usize>,
    },
    RadioBegin,
    RadioAccept {
        generation: u64,
        entries: Vec<Value>,
        exhausted: bool,
    },
    RadioFail {
        generation: u64,
    },
    RadioNext,
    RadioPending,
    CancelWaiting,
    Sort {
        values: Vec<SortValue>,
        descending: bool,
    },
    SortRows {
        rows: Vec<SortRow>,
        column: String,
        descending: bool,
    },
    FilterRows {
        rows: Vec<SortRow>,
        query: String,
    },
}

#[derive(Debug, Serialize)]
pub struct Reply {
    pub decision: Option<PlaybackDecision>,
    pub entry: Option<Value>,
    pub indices: Option<Vec<usize>>,
    pub accepted: bool,
    pub shuffle: ShuffleMode,
    pub repeat: RepeatMode,
    pub queued: Vec<usize>,
    pub stop_after: Vec<usize>,
    pub radio: RadioSnapshot,
}

#[derive(Debug, Serialize)]
pub struct RadioSnapshot {
    pub enabled: bool,
    pub generation: u64,
    pub ready: usize,
    pub waiting: bool,
    pub pending: bool,
    pub exhausted: bool,
    pub needs_refill: bool,
}

impl PolicyState {
    pub fn apply(&mut self, command: Command) -> Reply {
        let mut decision = None;
        let mut entry = None;
        let mut indices = None;
        let mut accepted = true;
        match command {
            Command::Init {
                seed,
                shuffle,
                repeat,
            } => {
                *self = Self::default();
                self.order = PlaybackOrder::new(shuffle, repeat, seed);
            }
            Command::Sync {
                tracks,
                current,
                old_to_new,
            } => {
                let ids: Vec<_> = tracks.iter().map(|track| track.id.clone()).collect();
                let metadata: Vec<_> = tracks.into_iter().map(|track| track.order).collect();
                if ids != self.ids || old_to_new.is_some() {
                    let remap =
                        old_to_new.unwrap_or_else(|| crate::remap_track_ids(&self.ids, &ids));
                    self.order.remap_tracks(&remap);
                    self.order.tracks_changed(&metadata, current);
                } else if metadata != self.tracks {
                    self.order.album_metadata_changed(&metadata, current);
                }
                self.ids = ids;
                self.tracks = metadata;
            }
            Command::Navigate { event, current } => {
                decision = Some(self.order.navigate(&self.tracks, current, event));
            }
            Command::Started { previous, index } => self.order.started(previous, index),
            Command::RadioCandidate { index } => self.order.radio_candidate(index),
            Command::CancelNavigation => self.order.cancel_navigation(),
            Command::SetShuffle { mode, current } => {
                self.order.set_shuffle_mode(mode, &self.tracks, current)
            }
            Command::CycleShuffle { current } => {
                self.order
                    .set_shuffle_mode(self.order.shuffle_mode().next(), &self.tracks, current)
            }
            Command::SetRepeat { mode } => self.order.set_repeat_mode(mode),
            Command::CycleRepeat => self.order.set_repeat_mode(self.order.repeat_mode().next()),
            Command::ToggleQueue { indices } => self.order.toggle_queue(&indices),
            Command::ToggleStopAfter { indices } => self.order.toggle_stop_after(&indices),
            Command::ClearQueue => self.order.clear_queue(),
            Command::RadioReset { enabled, current } => {
                self.order.set_radio_enabled(enabled, &self.tracks, current);
                self.radio.reset(enabled);
            }
            Command::RadioBegin => {
                self.radio.begin_request();
            }
            Command::RadioAccept {
                generation,
                entries,
                exhausted,
            } => accepted = self.radio.accept(generation, entries, exhausted),
            Command::RadioFail { generation } => accepted = self.radio.fail(generation),
            Command::RadioNext => entry = self.radio.request_next(),
            Command::RadioPending => entry = self.radio.take_pending(),
            Command::CancelWaiting => self.radio.cancel_waiting(),
            Command::Sort { values, descending } => {
                indices = Some(sorted_indices(&values, descending))
            }
            Command::SortRows {
                rows,
                column,
                descending,
            } => indices = Some(sorted_rows(&rows, &column, descending)),
            Command::FilterRows { rows, query } => {
                indices = Some(
                    rows.iter()
                        .enumerate()
                        .filter_map(|(index, row)| row.matches(&query).then_some(index))
                        .collect(),
                )
            }
        }
        if self.radio.enabled() && !self.order.radio_enabled() {
            self.radio.reset(false);
        }
        let mut stop_after: Vec<_> = self.order.stop_after_indices().collect();
        stop_after.sort_unstable();
        Reply {
            decision,
            entry,
            indices,
            accepted,
            shuffle: self.order.shuffle_mode(),
            repeat: self.order.repeat_mode(),
            queued: self.order.queued_indices().to_vec(),
            stop_after,
            radio: RadioSnapshot {
                enabled: self.radio.enabled(),
                generation: self.radio.generation(),
                ready: self.radio.ready_len(),
                waiting: self.radio.waiting(),
                pending: self.radio.pending(),
                exhausted: self.radio.exhausted(),
                needs_refill: self.radio.needs_refill(),
            },
        }
    }
}

/// The state string is opaque to the platform adapter. No native handles or
/// cross-language object lifetimes are required, and the wire path can be
/// replayed by exactly the same conformance tests as direct Rust calls.
pub fn dispatch_json(input: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Request {
        #[serde(default)]
        state: Option<String>,
        command: Command,
    }
    #[derive(Serialize)]
    struct Response {
        state: String,
        #[serde(flatten)]
        reply: Reply,
    }
    let request: Request = serde_json::from_str(input).map_err(|error| error.to_string())?;
    let mut state: PolicyState = request
        .state
        .as_deref()
        .filter(|state| !state.is_empty())
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| format!("Invalid playback state: {error}"))?
        .unwrap_or_default();
    let reply = state.apply(request.command);
    let state = serde_json::to_string(&state).map_err(|error| error.to_string())?;
    serde_json::to_string(&Response { state, reply }).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_bridge_replays_the_native_policy_without_different_ordering() {
        let mut native = PolicyState::default();
        let mut wire_state = None;
        let tracks: Vec<_> = (0..12)
            .map(|index| PolicyTrack {
                id: index.to_string(),
                order: OrderTrack {
                    album: format!("Album {}", index / 3),
                    track_number: Some(index + 1),
                    ..OrderTrack::default()
                },
            })
            .collect();
        let mut commands = vec![
            Command::Init {
                seed: 417,
                shuffle: ShuffleMode::Albums,
                repeat: RepeatMode::All,
            },
            Command::Sync {
                tracks,
                current: None,
                old_to_new: None,
            },
            Command::ToggleQueue {
                indices: vec![8, 4],
            },
            Command::ToggleStopAfter { indices: vec![2] },
        ];
        for index in 0..12 {
            commands.push(Command::Navigate {
                event: NavigationEvent::Next,
                current: Some(index),
            });
            commands.push(Command::Navigate {
                event: NavigationEvent::Failed,
                current: Some(index),
            });
        }
        commands.extend([
            Command::RadioReset {
                enabled: true,
                current: Some(11),
            },
            Command::RadioNext,
            Command::RadioAccept {
                generation: 1,
                entries: vec![serde_json::json!({"path":"game.hes","fragment":"10"})],
                exhausted: false,
            },
            Command::RadioPending,
            Command::SetRepeat {
                mode: RepeatMode::One,
            },
        ]);
        for command in commands {
            let input = serde_json::json!({"state": wire_state, "command": command});
            let mut wire: Value =
                serde_json::from_str(&dispatch_json(&input.to_string()).unwrap()).unwrap();
            wire_state = wire.as_object_mut().unwrap().remove("state");
            let direct = serde_json::to_value(native.apply(command)).unwrap();
            assert_eq!(wire, direct);
        }
    }
}
