//! A chain of output effects after the equalizer: reverb, stereo widener,
//! echo, chorus, warmth, bitcrusher, distortion and compressor. Each effect
//! can be switched on or off, moved, and adjusted; the whole chain is saved
//! as JSON and comes with presets.

use std::collections::BTreeMap;
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, SampleRate, Source};
use serde_json::{Value, json};

use crate::patch::Patch;

/// The effects Kog offers, in the order the catalog lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Reverb,
    Widener,
    Delay,
    Chorus,
    Warmth,
    Bitcrusher,
    Distortion,
    Compressor,
    /// A patch built from blocks; the slot names it.
    Custom,
}

pub const EFFECT_KINDS: [EffectKind; 8] = [
    EffectKind::Reverb,
    EffectKind::Widener,
    EffectKind::Delay,
    EffectKind::Chorus,
    EffectKind::Warmth,
    EffectKind::Bitcrusher,
    EffectKind::Distortion,
    EffectKind::Compressor,
];

/// One adjustable setting of an effect.
#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub step: f32,
    pub unit: &'static str,
    /// Names of the values of a setting that picks one of several, by index.
    pub choices: &'static [&'static str],
}

impl ParamSpec {
    /// A value brought into range (and to a whole choice).
    pub fn clamp(&self, value: f32) -> f32 {
        let value = value.clamp(self.min, self.max);
        if self.choices.is_empty() { value } else { value.round() }
    }
}

/// A setting that picks one of `choices`.
pub(crate) const fn choice(id: &'static str, label: &'static str, choices: &'static [&'static str], default: usize) -> ParamSpec {
    ParamSpec {
        id,
        label,
        min: 0.0,
        max: (choices.len() - 1) as f32,
        default: default as f32,
        step: 1.0,
        unit: "",
        choices,
    }
}

pub(crate) const fn param(
    id: &'static str,
    label: &'static str,
    min: f32,
    max: f32,
    default: f32,
    step: f32,
    unit: &'static str,
) -> ParamSpec {
    ParamSpec {
        id,
        label,
        min,
        max,
        default,
        step,
        unit,
        choices: &[],
    }
}

impl EffectKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Reverb => "reverb",
            Self::Widener => "widener",
            Self::Delay => "delay",
            Self::Chorus => "chorus",
            Self::Warmth => "warmth",
            Self::Bitcrusher => "bitcrusher",
            Self::Distortion => "distortion",
            Self::Compressor => "compressor",
            Self::Custom => "custom",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Reverb => "Reverb",
            Self::Widener => "Stereo widener",
            Self::Delay => "Echo",
            Self::Chorus => "Chorus",
            Self::Warmth => "Warmth",
            Self::Bitcrusher => "Bitcrusher",
            Self::Distortion => "Distortion",
            Self::Compressor => "Compressor",
            Self::Custom => "Custom effect",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        EFFECT_KINDS.into_iter().chain([Self::Custom]).find(|kind| kind.id() == id)
    }

    pub fn params(self) -> &'static [ParamSpec] {
        match self {
            Self::Reverb => {
                const P: &[ParamSpec] = &[
                    param("size", "Room size", 0.0, 1.0, 0.6, 0.01, ""),
                    param("damping", "Damping", 0.0, 1.0, 0.5, 0.01, ""),
                    param("width", "Width", 0.0, 1.0, 1.0, 0.01, ""),
                    param("mix", "Mix", 0.0, 1.0, 0.25, 0.01, ""),
                ];
                P
            }
            Self::Widener => {
                const P: &[ParamSpec] = &[param("width", "Width", 0.0, 2.0, 1.5, 0.01, "×")];
                P
            }
            Self::Delay => {
                const P: &[ParamSpec] = &[
                    param("time", "Time", 20.0, 1500.0, 350.0, 1.0, "ms"),
                    param("feedback", "Feedback", 0.0, 0.95, 0.35, 0.01, ""),
                    param("ping_pong", "Ping-pong", 0.0, 1.0, 1.0, 1.0, ""),
                    param("mix", "Mix", 0.0, 1.0, 0.3, 0.01, ""),
                ];
                P
            }
            Self::Chorus => {
                const P: &[ParamSpec] = &[
                    param("rate", "Rate", 0.05, 5.0, 0.8, 0.01, "Hz"),
                    param("depth", "Depth", 0.0, 10.0, 3.0, 0.1, "ms"),
                    param("delay", "Delay", 5.0, 30.0, 12.0, 0.1, "ms"),
                    param("mix", "Mix", 0.0, 1.0, 0.4, 0.01, ""),
                ];
                P
            }
            Self::Warmth => {
                const P: &[ParamSpec] = &[
                    param("bass", "Bass boost", 0.0, 12.0, 4.0, 0.1, "dB"),
                    param("cutoff", "High cut", 1000.0, 20000.0, 9000.0, 10.0, "Hz"),
                ];
                P
            }
            Self::Bitcrusher => {
                const P: &[ParamSpec] = &[
                    param("bits", "Bits", 2.0, 16.0, 8.0, 1.0, ""),
                    param("rate", "Sample rate", 1000.0, 48000.0, 11025.0, 1.0, "Hz"),
                    param("mix", "Mix", 0.0, 1.0, 1.0, 0.01, ""),
                ];
                P
            }
            Self::Distortion => {
                const P: &[ParamSpec] = &[
                    param("drive", "Drive", 0.0, 40.0, 12.0, 0.1, "dB"),
                    param("tone", "Tone", 500.0, 20000.0, 6000.0, 10.0, "Hz"),
                    param("mix", "Mix", 0.0, 1.0, 0.6, 0.01, ""),
                ];
                P
            }
            Self::Compressor => {
                const P: &[ParamSpec] = &[
                    param("threshold", "Threshold", -40.0, 0.0, -18.0, 0.1, "dB"),
                    param("ratio", "Ratio", 1.0, 20.0, 4.0, 0.1, ":1"),
                    param("attack", "Attack", 0.1, 100.0, 5.0, 0.1, "ms"),
                    param("release", "Release", 10.0, 1000.0, 120.0, 1.0, "ms"),
                    param("makeup", "Makeup gain", 0.0, 24.0, 4.0, 0.1, "dB"),
                    param("ceiling", "Limiter ceiling", -12.0, 0.0, -1.0, 0.1, "dB"),
                ];
                P
            }
            Self::Custom => {
                const P: &[ParamSpec] = &[param("mix", "Mix", 0.0, 1.0, 1.0, 0.01, "")];
                P
            }
        }
    }
}

/// One effect in the chain.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectSlot {
    pub kind: EffectKind,
    pub enabled: bool,
    pub params: BTreeMap<String, f32>,
    /// For a custom effect: the name of its patch.
    pub patch: Option<String>,
}

impl EffectSlot {
    /// An effect with its default settings, switched on.
    pub fn new(kind: EffectKind) -> Self {
        Self {
            kind,
            enabled: true,
            params: kind
                .params()
                .iter()
                .map(|spec| (spec.id.to_owned(), spec.default))
                .collect(),
            patch: None,
        }
    }

    /// A custom effect running the named patch.
    pub fn custom(patch: &str) -> Self {
        Self { patch: Some(patch.to_owned()), ..Self::new(EffectKind::Custom) }
    }

    /// A setting, clamped to its range; the default when it is missing.
    pub fn get(&self, id: &str) -> f32 {
        let Some(spec) = self.kind.params().iter().find(|spec| spec.id == id) else {
            return 0.0;
        };
        self.params
            .get(id)
            .copied()
            .filter(|value| value.is_finite())
            .map_or(spec.default, |value| spec.clamp(value))
    }

    fn with(mut self, values: &[(&str, f32)]) -> Self {
        for (id, value) in values {
            self.params.insert((*id).to_owned(), *value);
        }
        self
    }
}

/// The whole chain, in processing order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectsSettings {
    pub enabled: bool,
    pub preset_name: String,
    pub chain: Vec<EffectSlot>,
    /// Effects built from blocks, by name; slots refer to them.
    pub patches: Vec<Patch>,
}

/// Effects in a chain at most; each kind may appear more than once.
pub const MAX_EFFECTS: usize = 16;
/// Saved custom effects at most.
pub const MAX_PATCHES: usize = 64;

impl EffectsSettings {
    pub fn to_json(&self) -> Value {
        json!({
            "version": 1,
            "enabled": self.enabled,
            "preset": self.preset_name,
            "chain": self.chain.iter().map(|slot| json!({
                "kind": slot.kind.id(),
                "enabled": slot.enabled,
                "params": slot.kind.params().iter().map(|spec| (spec.id.to_owned(), json!(slot.get(spec.id)))).collect::<serde_json::Map<_, _>>(),
                "patch": slot.patch,
            })).collect::<Vec<_>>(),
            "patches": self.patches.iter().map(Patch::to_json).collect::<Vec<_>>(),
        })
    }

    pub fn serialize(&self) -> String {
        self.to_json().to_string()
    }

    /// Read a chain, ignoring unknown effects and settings and clamping the
    /// rest to their ranges.
    pub fn from_json(value: &Value) -> Option<Self> {
        let chain = value.get("chain")?.as_array()?;
        Some(Self {
            enabled: value
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            preset_name: value
                .get("preset")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect(),
            chain: chain
                .iter()
                .filter_map(|slot| {
                    let kind = EffectKind::from_id(slot.get("kind")?.as_str()?)?;
                    let mut effect = EffectSlot::new(kind);
                    effect.enabled = slot.get("enabled").and_then(Value::as_bool).unwrap_or(true);
                    if kind == EffectKind::Custom {
                        effect.patch = Some(slot.get("patch")?.as_str()?.chars().take(60).collect());
                    }
                    for spec in kind.params() {
                        if let Some(value) = slot
                            .get("params")
                            .and_then(|params| params.get(spec.id))
                            .and_then(Value::as_f64)
                        {
                            let value = value as f32;
                            if value.is_finite() {
                                effect
                                    .params
                                    .insert(spec.id.to_owned(), spec.clamp(value));
                            }
                        }
                    }
                    Some(effect)
                })
                .take(MAX_EFFECTS)
                .collect(),
            patches: {
                let mut patches: Vec<Patch> = value
                    .get("patches")
                    .and_then(Value::as_array)
                    .map(|items| items.iter().filter_map(Patch::from_json).take(MAX_PATCHES).collect())
                    .unwrap_or_default();
                // Names are how slots find patches, so keep them unique.
                let mut seen = std::collections::HashSet::new();
                patches.retain(|patch| seen.insert(patch.name.clone()));
                patches
            },
        })
    }

    pub fn parse(text: &str) -> Option<Self> {
        if text.len() > 1024 * 1024 {
            return None;
        }
        Self::from_json(&serde_json::from_str(text).ok()?)
    }

    /// Whether anything would change the sound.
    pub fn active(&self) -> bool {
        self.enabled && self.chain.iter().any(|slot| slot.enabled)
    }
}

/// Ready-made chains, starting with no effects.
pub fn presets() -> Vec<EffectsSettings> {
    use EffectKind::*;
    let preset = |name: &str, chain: Vec<EffectSlot>| EffectsSettings {
        enabled: true,
        preset_name: name.to_owned(),
        chain,
        patches: Vec::new(),
    };
    vec![
        preset("None", vec![]),
        preset(
            "Room",
            vec![EffectSlot::new(Reverb).with(&[("size", 0.45), ("damping", 0.55), ("mix", 0.18)])],
        ),
        preset(
            "Hall",
            vec![EffectSlot::new(Reverb).with(&[("size", 0.85), ("damping", 0.4), ("mix", 0.3)])],
        ),
        preset(
            "Wide chip",
            vec![
                EffectSlot::new(Chorus).with(&[("rate", 0.4), ("depth", 2.0), ("mix", 0.25)]),
                EffectSlot::new(Widener).with(&[("width", 1.6)]),
                EffectSlot::new(Reverb).with(&[("size", 0.4), ("mix", 0.15)]),
            ],
        ),
        preset(
            "Lo-fi handheld",
            vec![
                EffectSlot::new(Bitcrusher).with(&[("bits", 8.0), ("rate", 16384.0)]),
                EffectSlot::new(Warmth).with(&[("bass", 0.0), ("cutoff", 5000.0)]),
                EffectSlot::new(Widener).with(&[("width", 0.0)]),
            ],
        ),
        preset(
            "Arcade cabinet",
            vec![
                EffectSlot::new(Warmth).with(&[("bass", 6.0), ("cutoff", 7000.0)]),
                EffectSlot::new(Reverb).with(&[("size", 0.3), ("damping", 0.7), ("mix", 0.12)]),
                EffectSlot::new(Compressor),
            ],
        ),
        preset(
            "Echo chamber",
            vec![
                EffectSlot::new(Delay),
                EffectSlot::new(Reverb).with(&[("size", 0.7), ("mix", 0.2)]),
            ],
        ),
        preset(
            "Overdrive",
            vec![
                EffectSlot::new(Distortion),
                EffectSlot::new(Compressor).with(&[("threshold", -12.0), ("makeup", 0.0)]),
            ],
        ),
    ]
}

/// What the settings screens need: every effect with its settings, and the
/// presets.
pub fn catalog() -> Value {
    json!({
        "effects": EFFECT_KINDS.iter().map(|kind| json!({
            "kind": kind.id(),
            "label": kind.label(),
            "params": kind.params().iter().map(|spec| json!({
                "id": spec.id, "label": spec.label, "min": spec.min, "max": spec.max,
                "default": spec.default, "step": spec.step, "unit": spec.unit,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "presets": presets().iter().map(EffectsSettings::to_json).collect::<Vec<_>>(),
        "custom": crate::patch::catalog(),
    })
}

/// Shared between the settings and the audio thread.
#[derive(Clone)]
pub struct EffectsControl {
    shared: Arc<EffectsShared>,
}

struct EffectsShared {
    settings: RwLock<EffectsSettings>,
    revision: AtomicU64,
}

impl EffectsControl {
    pub fn new(settings: EffectsSettings) -> Self {
        Self {
            shared: Arc::new(EffectsShared {
                settings: RwLock::new(settings),
                revision: AtomicU64::new(1),
            }),
        }
    }

    pub fn set(&self, settings: EffectsSettings) {
        *self
            .shared
            .settings
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = settings;
        self.shared.revision.fetch_add(1, Ordering::Release);
    }

    /// Clear tails (reverb, echo) when playback jumps.
    pub fn reset(&self) {
        self.shared.revision.fetch_add(1, Ordering::Release);
    }

    pub fn get(&self) -> EffectsSettings {
        self.shared
            .settings
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

// ---------------------------------------------------------------------------
// Building blocks

#[derive(Clone, Copy, Default)]
pub(crate) struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    pub(crate) z1: f32,
    pub(crate) z2: f32,
}

impl Biquad {
    fn from(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn low_pass(frequency: f32, rate: f32) -> Self {
        let w = TAU * frequency.min(rate * 0.45) / rate;
        let alpha = w.sin() / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
        let cos = w.cos();
        Self::from(
            (1.0 - cos) / 2.0,
            1.0 - cos,
            (1.0 - cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        )
    }

    fn low_shelf(frequency: f32, gain_db: f32, rate: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w = TAU * frequency / rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / 2.0 * std::f32::consts::SQRT_2;
        let root = 2.0 * a.sqrt() * alpha;
        Self::from(
            a * ((a + 1.0) - (a - 1.0) * cos + root),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
            a * ((a + 1.0) - (a - 1.0) * cos - root),
            (a + 1.0) + (a - 1.0) * cos + root,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos),
            (a + 1.0) + (a - 1.0) * cos - root,
        )
    }

    /// The RBJ cookbook filters, by `FILTER_TYPES` index.
    pub(crate) fn design(kind: usize, frequency: f32, q: f32, gain_db: f32, rate: f32) -> Self {
        let w = TAU * frequency.clamp(10.0, rate * 0.45) / rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q.max(0.05));
        let a = 10f32.powf(gain_db / 40.0);
        match kind {
            1 => Self::from((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            2 => Self::from(alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            3 => Self::from(1.0, -2.0 * cos, 1.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            4 => Self::from(1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a),
            5 | 6 => {
                let root = 2.0 * a.sqrt() * alpha;
                let sign = if kind == 5 { 1.0 } else { -1.0 };
                Self::from(
                    a * ((a + 1.0) - sign * (a - 1.0) * cos + root),
                    sign * 2.0 * a * ((a - 1.0) - sign * (a + 1.0) * cos),
                    a * ((a + 1.0) - sign * (a - 1.0) * cos - root),
                    (a + 1.0) + sign * (a - 1.0) * cos + root,
                    -sign * 2.0 * ((a - 1.0) + sign * (a + 1.0) * cos),
                    (a + 1.0) + sign * (a - 1.0) * cos - root,
                )
            }
            7 => Self::from(1.0 - alpha, -2.0 * cos, 1.0 + alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            _ => Self::from((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
        }
    }

    pub(crate) fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// A delay line read at a fractional position behind the write head.
pub(crate) struct DelayLine {
    buffer: Vec<f32>,
    write: usize,
}

impl DelayLine {
    pub(crate) fn new(length: usize) -> Self {
        Self {
            buffer: vec![0.0; length.max(2)],
            write: 0,
        }
    }

    pub(crate) fn push(&mut self, value: f32) {
        self.buffer[self.write] = value;
        self.write = (self.write + 1) % self.buffer.len();
    }

    /// The sample `delay` samples ago (before the latest push).
    pub(crate) fn read(&self, delay: f32) -> f32 {
        let length = self.buffer.len();
        let delay = delay.clamp(1.0, (length - 2) as f32);
        let whole = delay.floor() as usize;
        let fraction = delay - whole as f32;
        let at = |back: usize| self.buffer[(self.write + length - back) % length];
        at(whole) * (1.0 - fraction) + at(whole + 1) * fraction
    }
}

struct Comb {
    line: Vec<f32>,
    at: usize,
    store: f32,
}

impl Comb {
    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let output = self.line[self.at];
        self.store = output * (1.0 - damp) + self.store * damp;
        self.line[self.at] = input + self.store * feedback;
        self.at = (self.at + 1) % self.line.len();
        output
    }
}

struct Allpass {
    line: Vec<f32>,
    at: usize,
}

impl Allpass {
    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.line[self.at];
        let output = delayed - input;
        self.line[self.at] = input + delayed * 0.5;
        self.at = (self.at + 1) % self.line.len();
        output
    }
}

// ---------------------------------------------------------------------------
// Effects

pub(crate) trait Effect: Send {
    fn process(&mut self, left: f32, right: f32) -> (f32, f32);
}

/// Freeverb: eight damped combs and four allpasses per side.
struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
    feedback: f32,
    damp: f32,
    wet1: f32,
    wet2: f32,
    dry: f32,
}

impl Reverb {
    fn new(slot: &EffectSlot, rate: f32) -> Self {
        const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
        const SPREAD: usize = 23;
        let scale = rate / 44_100.0;
        let size = |samples: usize| ((samples as f32 * scale) as usize).max(1);
        let side = |spread: usize| {
            (
                COMBS
                    .iter()
                    .map(|length| Comb {
                        line: vec![0.0; size(length + spread)],
                        at: 0,
                        store: 0.0,
                    })
                    .collect::<Vec<_>>(),
                ALLPASSES
                    .iter()
                    .map(|length| Allpass {
                        line: vec![0.0; size(length + spread)],
                        at: 0,
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let (left_combs, left_allpasses) = side(0);
        let (right_combs, right_allpasses) = side(SPREAD);
        let mix = slot.get("mix");
        let width = slot.get("width");
        let wet = mix * 3.0;
        Self {
            combs: [left_combs, right_combs],
            allpasses: [left_allpasses, right_allpasses],
            feedback: 0.7 + slot.get("size") * 0.28,
            damp: slot.get("damping") * 0.4,
            wet1: wet * (width / 2.0 + 0.5),
            wet2: wet * ((1.0 - width) / 2.0),
            dry: 1.0 - mix,
        }
    }
}

impl Effect for Reverb {
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let input = (left + right) * 0.015;
        let mut out = [0.0f32; 2];
        for side in 0..2 {
            let mut sum = 0.0;
            for comb in &mut self.combs[side] {
                sum += comb.process(input, self.feedback, self.damp);
            }
            for allpass in &mut self.allpasses[side] {
                sum = allpass.process(sum);
            }
            out[side] = sum;
        }
        (
            out[0] * self.wet1 + out[1] * self.wet2 + left * self.dry,
            out[1] * self.wet1 + out[0] * self.wet2 + right * self.dry,
        )
    }
}

/// Mid/side width: 0 is mono, 1 unchanged, 2 twice as wide.
struct Widener {
    width: f32,
}

impl Effect for Widener {
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let mid = (left + right) * 0.5;
        let side = (left - right) * 0.5 * self.width;
        // Keep loudness steady as the image widens.
        let gain = 1.0 / (1.0 + (self.width - 1.0).max(0.0) * 0.25);
        ((mid + side) * gain, (mid - side) * gain)
    }
}

/// Echo with feedback; ping-pong bounces repeats between the sides.
struct Delay {
    lines: [DelayLine; 2],
    delay: f32,
    feedback: f32,
    ping_pong: bool,
    mix: f32,
    tone: [Biquad; 2],
}

impl Delay {
    fn new(slot: &EffectSlot, rate: f32) -> Self {
        let delay = slot.get("time") / 1000.0 * rate;
        let length = delay as usize + 4;
        Self {
            lines: [DelayLine::new(length), DelayLine::new(length)],
            delay,
            feedback: slot.get("feedback"),
            ping_pong: slot.get("ping_pong") >= 0.5,
            mix: slot.get("mix"),
            // Each repeat is a little duller, like tape.
            tone: [Biquad::low_pass(6000.0, rate); 2],
        }
    }
}

impl Effect for Delay {
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let echo = [
            self.lines[0].read(self.delay),
            self.lines[1].read(self.delay),
        ];
        let feed = [
            self.tone[0].process(echo[0]) * self.feedback,
            self.tone[1].process(echo[1]) * self.feedback,
        ];
        if self.ping_pong {
            self.lines[0].push((left + right) * 0.5 + feed[1]);
            self.lines[1].push(feed[0]);
        } else {
            self.lines[0].push(left + feed[0]);
            self.lines[1].push(right + feed[1]);
        }
        (left + echo[0] * self.mix, right + echo[1] * self.mix)
    }
}

/// A delay swept by a slow wave, a quarter cycle apart on each side.
struct Chorus {
    lines: [DelayLine; 2],
    phase: f32,
    step: f32,
    base: f32,
    depth: f32,
    mix: f32,
}

impl Chorus {
    fn new(slot: &EffectSlot, rate: f32) -> Self {
        let base = slot.get("delay") / 1000.0 * rate;
        let depth = slot.get("depth") / 1000.0 * rate;
        let length = (base + depth) as usize + 4;
        Self {
            lines: [DelayLine::new(length), DelayLine::new(length)],
            phase: 0.0,
            step: slot.get("rate") / rate,
            base,
            depth,
            mix: slot.get("mix"),
        }
    }
}

impl Effect for Chorus {
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.lines[0].push(left);
        self.lines[1].push(right);
        self.phase = (self.phase + self.step).fract();
        let sweep = |offset: f32| (TAU * (self.phase + offset)).sin() * 0.5 + 0.5;
        let wet_left = self.lines[0].read(self.base + self.depth * sweep(0.0));
        let wet_right = self.lines[1].read(self.base + self.depth * sweep(0.25));
        let dry = 1.0 - self.mix * 0.5;
        (
            left * dry + wet_left * self.mix,
            right * dry + wet_right * self.mix,
        )
    }
}

/// A low shelf for weight and a gentle high cut, like an old speaker.
struct Warmth {
    shelf: [Biquad; 2],
    cut: [Biquad; 2],
}

impl Effect for Warmth {
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        (
            self.cut[0].process(self.shelf[0].process(left)),
            self.cut[1].process(self.shelf[1].process(right)),
        )
    }
}

/// Fewer bits and a lower sample rate, held between samples.
struct Bitcrusher {
    levels: f32,
    step: f32,
    phase: f32,
    held: (f32, f32),
    mix: f32,
}

impl Effect for Bitcrusher {
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            let crush = |value: f32| (value.clamp(-1.0, 1.0) * self.levels).round() / self.levels;
            self.held = (crush(left), crush(right));
        }
        (
            left + (self.held.0 - left) * self.mix,
            right + (self.held.1 - right) * self.mix,
        )
    }
}

/// Soft clipping after a drive gain, then a tone filter.
struct Distortion {
    drive: f32,
    makeup: f32,
    tone: [Biquad; 2],
    mix: f32,
}

impl Effect for Distortion {
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let shape = |value: f32| (value * self.drive).tanh() * self.makeup;
        let wet = (
            self.tone[0].process(shape(left)),
            self.tone[1].process(shape(right)),
        );
        (
            left + (wet.0 - left) * self.mix,
            right + (wet.1 - right) * self.mix,
        )
    }
}

/// A feed-forward compressor on the louder side, then a soft limiter.
struct Compressor {
    threshold_db: f32,
    slope: f32,
    attack: f32,
    release: f32,
    makeup: f32,
    ceiling: f32,
    envelope_db: f32,
}

impl Effect for Compressor {
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let peak = left.abs().max(right.abs()).max(1e-6);
        let level_db = 20.0 * peak.log10();
        let coefficient = if level_db > self.envelope_db {
            self.attack
        } else {
            self.release
        };
        self.envelope_db = level_db + coefficient * (self.envelope_db - level_db);
        let over = self.envelope_db - self.threshold_db;
        let reduction_db = if over > 0.0 { over * self.slope } else { 0.0 };
        let gain = 10f32.powf(-reduction_db / 20.0) * self.makeup;
        let limit = |value: f32| {
            let value = value * gain;
            if value.abs() <= self.ceiling * 0.8 {
                value
            } else {
                // Round off the last fifth below the ceiling.
                let knee = self.ceiling * 0.8;
                value.signum()
                    * (knee
                        + (self.ceiling - knee)
                            * ((value.abs() - knee) / (self.ceiling - knee)).tanh())
            }
        };
        (limit(left), limit(right))
    }
}

fn build(slot: &EffectSlot, rate: f32, patches: &[Patch]) -> Option<Box<dyn Effect>> {
    let time = |ms: f32| (-1.0 / (ms / 1000.0 * rate)).exp();
    Some(match slot.kind {
        EffectKind::Custom => {
            let name = slot.patch.as_deref()?;
            let patch = patches.iter().find(|patch| patch.name == name)?;
            Box::new(crate::patch::PatchEffect::new(patch, rate, slot.get("mix")))
        }
        EffectKind::Reverb => Box::new(Reverb::new(slot, rate)),
        EffectKind::Widener => Box::new(Widener {
            width: slot.get("width"),
        }),
        EffectKind::Delay => Box::new(Delay::new(slot, rate)),
        EffectKind::Chorus => Box::new(Chorus::new(slot, rate)),
        EffectKind::Warmth => Box::new(Warmth {
            shelf: [Biquad::low_shelf(120.0, slot.get("bass"), rate); 2],
            cut: [Biquad::low_pass(slot.get("cutoff"), rate); 2],
        }),
        EffectKind::Bitcrusher => Box::new(Bitcrusher {
            levels: 2f32.powf(slot.get("bits") - 1.0),
            step: (slot.get("rate") / rate).min(1.0),
            phase: 1.0,
            held: (0.0, 0.0),
            mix: slot.get("mix"),
        }),
        EffectKind::Distortion => {
            let drive = 10f32.powf(slot.get("drive") / 20.0);
            Box::new(Distortion {
                drive,
                // Keep the level about the same as the drive rises.
                makeup: 1.0 / drive.tanh().max(0.05),
                tone: [Biquad::low_pass(slot.get("tone"), rate); 2],
                mix: slot.get("mix"),
            })
        }
        EffectKind::Compressor => Box::new(Compressor {
            threshold_db: slot.get("threshold"),
            slope: 1.0 - 1.0 / slot.get("ratio"),
            attack: time(slot.get("attack")),
            release: time(slot.get("release")),
            makeup: 10f32.powf(slot.get("makeup") / 20.0),
            ceiling: 10f32.powf(slot.get("ceiling") / 20.0),
            envelope_db: -120.0,
        }),
    })
}

// ---------------------------------------------------------------------------
// Source

/// Runs the chain on interleaved audio. Mono is processed as a centred pair
/// and folded back; channels past the first two pass through unchanged.
pub struct EffectsSource<S> {
    input: S,
    control: EffectsControl,
    revision: u64,
    effects: Vec<Box<dyn Effect>>,
    channels: ChannelCount,
    sample_rate: SampleRate,
    frame: Vec<f32>,
    output: Vec<f32>,
    cursor: usize,
}

impl<S: Source<Item = f32>> EffectsSource<S> {
    pub fn new(input: S, control: EffectsControl) -> Self {
        let channels = input.channels();
        let sample_rate = input.sample_rate();
        let mut source = Self {
            input,
            control,
            revision: 0,
            effects: Vec::new(),
            channels,
            sample_rate,
            frame: Vec::with_capacity(usize::from(channels.get())),
            output: Vec::new(),
            cursor: 0,
        };
        source.refresh();
        source
    }

    fn refresh(&mut self) {
        let revision = self.control.shared.revision.load(Ordering::Acquire);
        if revision == self.revision {
            return;
        }
        self.revision = revision;
        let settings = self.control.get();
        let rate = self.sample_rate.get() as f32;
        self.effects = if settings.enabled {
            settings
                .chain
                .iter()
                .filter(|slot| slot.enabled)
                .filter_map(|slot| build(slot, rate, &settings.patches))
                .collect()
        } else {
            Vec::new()
        };
    }

    fn next_frame(&mut self) -> bool {
        self.refresh();
        self.frame.clear();
        for _ in 0..self.channels.get() {
            match self.input.next() {
                Some(sample) => self.frame.push(sample),
                None => break,
            }
        }
        if self.frame.is_empty() {
            return false;
        }
        self.output.clear();
        self.output.extend_from_slice(&self.frame);
        if !self.effects.is_empty() && self.frame.len() == usize::from(self.channels.get()) {
            let (mut left, mut right) = if self.frame.len() >= 2 {
                (self.frame[0], self.frame[1])
            } else {
                (self.frame[0], self.frame[0])
            };
            for effect in &mut self.effects {
                (left, right) = effect.process(left, right);
            }
            // Never hand the device a value it would wrap or a NaN.
            let safe = |value: f32| {
                if value.is_finite() {
                    value.clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            };
            if self.output.len() >= 2 {
                self.output[0] = safe(left);
                self.output[1] = safe(right);
            } else {
                self.output[0] = safe((left + right) * 0.5);
            }
        }
        self.cursor = 0;
        true
    }
}

impl<S: Source<Item = f32>> Iterator for EffectsSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.cursor >= self.output.len() && !self.next_frame() {
            return None;
        }
        let sample = self.output[self.cursor];
        self.cursor += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<S: Source<Item = f32>> Source for EffectsSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.input.try_seek(position)?;
        // Tails from before the jump would ring over the new position.
        self.revision = 0;
        self.output.clear();
        self.cursor = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;
    use std::num::NonZero;

    fn run(settings: EffectsSettings, samples: Vec<f32>) -> Vec<f32> {
        let source = SamplesBuffer::new(
            NonZero::new(2).unwrap(),
            NonZero::new(48_000).unwrap(),
            samples,
        );
        EffectsSource::new(source, EffectsControl::new(settings)).collect()
    }

    fn impulse() -> Vec<f32> {
        let mut samples = vec![0.0; 48_000 * 2];
        samples[0] = 0.8;
        samples[1] = 0.8;
        samples
    }

    #[test]
    fn no_chain_passes_audio_through_unchanged() {
        let samples: Vec<f32> = (0..2000).map(|i| ((i as f32) * 0.01).sin() * 0.5).collect();
        assert_eq!(
            run(
                EffectsSettings {
                    enabled: true,
                    patches: Vec::new(),
                    ..Default::default()
                },
                samples.clone()
            ),
            samples
        );
        let mut off = presets()[2].clone();
        off.enabled = false;
        assert_eq!(run(off, samples.clone()), samples);
    }

    #[test]
    fn every_effect_stays_finite_bounded_and_audible() {
        for kind in EFFECT_KINDS {
            let settings = EffectsSettings {
                enabled: true,
                patches: Vec::new(),
                preset_name: String::new(),
                chain: vec![EffectSlot::new(kind)],
            };
            let noise: Vec<f32> = (0..48_000)
                .map(|i| ((i * 7919 % 2003) as f32 / 1001.5 - 1.0) * 0.9)
                .collect();
            let output = run(settings.clone(), noise.clone());
            assert_eq!(output.len(), noise.len(), "{kind:?}");
            assert!(
                output
                    .iter()
                    .all(|value| value.is_finite() && value.abs() <= 1.0),
                "{kind:?}"
            );
            assert!(
                output.iter().zip(&noise).any(|(a, b)| (a - b).abs() > 1e-4),
                "{kind:?} changed nothing"
            );
        }
    }

    #[test]
    fn reverb_and_echo_leave_a_tail_after_an_impulse() {
        for kind in [EffectKind::Reverb, EffectKind::Delay] {
            let settings = EffectsSettings {
                enabled: true,
                patches: Vec::new(),
                preset_name: String::new(),
                chain: vec![EffectSlot::new(kind)],
            };
            let output = run(settings, impulse());
            let tail: f32 = output[48_000 / 2..].iter().map(|value| value.abs()).sum();
            assert!(tail > 0.01, "{kind:?} left no tail");
        }
    }

    #[test]
    fn settings_round_trip_through_json_and_clamp_bad_values() {
        for preset in presets() {
            assert_eq!(EffectsSettings::parse(&preset.serialize()), Some(preset));
        }
        let parsed = EffectsSettings::parse(
            r#"{"enabled":true,"chain":[{"kind":"reverb","params":{"mix":9}},{"kind":"nope"}]}"#,
        )
        .unwrap();
        assert_eq!(parsed.chain.len(), 1);
        assert_eq!(parsed.chain[0].get("mix"), 1.0);
        assert!(EffectsSettings::parse("not json").is_none());
    }

    #[test]
    fn custom_slots_run_their_patch_and_round_trip() {
        let patch = crate::patch::templates().remove(0);
        let settings = EffectsSettings {
            enabled: true,
            preset_name: String::new(),
            chain: vec![EffectSlot::custom(&patch.name), EffectSlot::custom("missing")],
            patches: vec![patch],
        };
        assert_eq!(EffectsSettings::parse(&settings.serialize()), Some(settings.clone()));
        let output = run(settings, impulse());
        let tail: f32 = output[4_000..].iter().map(|value| value.abs()).sum();
        assert!(tail > 0.01, "the reverb patch left no tail");
    }

    #[test]
    fn widener_at_zero_makes_mono() {
        let settings = EffectsSettings {
            enabled: true,
            patches: Vec::new(),
            preset_name: String::new(),
            chain: vec![EffectSlot::new(EffectKind::Widener).with(&[("width", 0.0)])],
        };
        let output = run(settings, vec![0.6, -0.2, 0.1, 0.3]);
        assert!((output[0] - output[1]).abs() < 1e-6 && (output[2] - output[3]).abs() < 1e-6);
    }

    #[test]
    fn catalog_lists_all_eight_effects_and_presets() {
        let catalog = catalog();
        assert_eq!(catalog["effects"].as_array().unwrap().len(), 8);
        assert!(catalog["presets"].as_array().unwrap().len() >= 5);
    }
}
