use super::{Channel, Field, FrameData, Note};

#[repr(C)]
#[derive(Clone)]
pub(crate) struct Voice {
    pub id: u32,
    pub kind: u32,
    pub active: u32,
    pub key: f32,
    pub level: f32,
    pub pan: f32,
    pub name: [u8; 64],
    pub instrument: [u8; 64],
    pub details: [u8; 512],
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            id: 0,
            kind: 0,
            active: 0,
            key: -1.0,
            level: 0.0,
            pan: 0.0,
            name: [0; 64],
            instrument: [0; 64],
            details: [0; 512],
        }
    }
}

fn string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(
        &bytes[..bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len())],
    )
    .into_owned()
}

pub(crate) fn frame(voices: &[Voice]) -> FrameData {
    FrameData {
        channels: voices
            .iter()
            .map(|voice| {
                let active = voice.active != 0;
                Channel {
                    id: voice.id,
                    name: string(&voice.name),
                    instrument: string(&voice.instrument),
                    kind: match voice.kind {
                        1 => "noise",
                        2 => "sample",
                        3 => "percussion",
                        4 => "mixed",
                        _ => "tonal",
                    }
                    .into(),
                    active,
                    level: voice.level.clamp(0.0, 1.0),
                    pan: voice.pan.clamp(-1.0, 1.0),
                    notes: if active && voice.key.is_finite() && voice.key >= 0.0 {
                        vec![Note {
                            key: voice.key,
                            velocity: voice.level.clamp(0.0, 1.0),
                            held: true,
                        }]
                    } else {
                        Vec::new()
                    },
                    fields: string(&voice.details)
                        .split(" | ")
                        .filter(|item| !item.is_empty())
                        .map(|item| {
                            let (name, value) = item.split_once('=').unwrap_or(("State", item));
                            Field::new(name, value)
                        })
                        .collect(),
                }
            })
            .collect(),
        ..FrameData::default()
    }
}

pub(crate) fn capture(capacity: usize, read: impl FnOnce(*mut Voice, usize) -> usize) -> FrameData {
    let mut voices = vec![Voice::default(); capacity];
    let count = read(voices.as_mut_ptr(), capacity).min(capacity);
    frame(&voices[..count])
}
