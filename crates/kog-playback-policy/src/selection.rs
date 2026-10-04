//! Selection gestures shared by every presentation adapter.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gesture {
    Replace,
    Toggle,
    Range,
    AddRange,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Choose {
        index: usize,
        gesture: Gesture,
    },
    All,
    Clear,
    Set {
        indices: Vec<usize>,
        anchor: Option<usize>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub indices: Vec<usize>,
    pub anchor: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Activation {
    Play { index: usize },
    TogglePlayback,
}

/// Activating the current queue row toggles output; another row starts there.
/// Gesture recognition (double click, Enter, or touch activation) is platform input.
pub fn activate(index: usize, current: Option<usize>, count: usize) -> Option<Activation> {
    (index < count).then_some(if current == Some(index) {
        Activation::TogglePlayback
    } else {
        Activation::Play { index }
    })
}
impl Selection {
    pub fn apply(&mut self, command: Command, count: usize, visible: &[usize]) {
        let order: Vec<_> = if visible.is_empty() {
            (0..count).collect()
        } else {
            visible.iter().copied().filter(|i| *i < count).collect()
        };
        self.indices.retain(|i| *i < count);
        self.anchor = self.anchor.filter(|i| *i < count);
        match command {
            Command::Clear => {
                self.indices.clear();
                self.anchor = None;
            }
            Command::All => {
                self.indices = order;
                self.anchor = self.indices.first().copied();
            }
            Command::Set { indices, anchor } => {
                self.indices = indices;
                self.anchor = anchor.filter(|i| *i < count);
            }
            Command::Choose { index, gesture } if index < count && order.contains(&index) => {
                match gesture {
                    Gesture::Replace => {
                        self.indices = vec![index];
                        self.anchor = Some(index);
                    }
                    Gesture::Toggle => {
                        if self.indices.contains(&index) {
                            self.indices.retain(|i| *i != index);
                        } else {
                            self.indices.push(index);
                        }
                        self.anchor = Some(index);
                    }
                    Gesture::Range | Gesture::AddRange => {
                        let anchor = self.anchor.filter(|a| order.contains(a)).unwrap_or(index);
                        let a = order.iter().position(|i| *i == anchor).unwrap();
                        let b = order.iter().position(|i| *i == index).unwrap();
                        if matches!(gesture, Gesture::Range) {
                            self.indices.clear();
                        }
                        self.indices.extend_from_slice(&order[a.min(b)..=a.max(b)]);
                        self.anchor = Some(anchor);
                    }
                }
            }
            Command::Choose { .. } => {}
        }
        self.indices.retain(|i| *i < count);
        self.indices.sort_unstable();
        self.indices.dedup();
    }
    pub fn remap(&mut self, old_to_new: &[Option<usize>]) {
        self.indices = self
            .indices
            .iter()
            .filter_map(|i| old_to_new.get(*i).copied().flatten())
            .collect();
        self.indices.sort_unstable();
        self.indices.dedup();
        self.anchor = self
            .anchor
            .and_then(|i| old_to_new.get(i).copied().flatten());
    }
}
