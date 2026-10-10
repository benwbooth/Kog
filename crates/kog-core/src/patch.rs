//! Build-your-own effects: a patch is a chain of building blocks (filters,
//! delays, gain, shapers …), with parallel paths, feedback loops, and
//! LFOs and envelope followers that can move any block's setting.
//!
//! A patch is a list of stages processed top to bottom. A stage is a block,
//! a parallel group (branches that each process the input and are summed at
//! their own levels), or a feedback loop (stages whose output is fed back to
//! their input). Stages nest, so a branch can hold its own groups.

use std::collections::BTreeMap;
use std::f32::consts::TAU;

use serde_json::{Value, json};

use crate::effects::{Biquad, DelayLine, Effect, ParamSpec, choice, param};

/// The building blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Filter,
    Delay,
    Comb,
    Allpass,
    Gain,
    Shaper,
    Crusher,
    Width,
    Pan,
    Dynamics,
    Ring,
}

pub const BLOCK_KINDS: [BlockKind; 11] = [
    BlockKind::Filter,
    BlockKind::Delay,
    BlockKind::Comb,
    BlockKind::Allpass,
    BlockKind::Gain,
    BlockKind::Shaper,
    BlockKind::Crusher,
    BlockKind::Width,
    BlockKind::Pan,
    BlockKind::Dynamics,
    BlockKind::Ring,
];

pub const FILTER_TYPES: &[&str] = &["Low-pass", "High-pass", "Band-pass", "Notch", "Peak", "Low shelf", "High shelf", "All-pass"];
pub const SHAPER_CURVES: &[&str] = &["Soft clip", "Hard clip", "Fold", "Sine", "Asymmetric"];
pub const LFO_SHAPES: &[&str] = &["Sine", "Triangle", "Square", "Saw", "Random"];

impl BlockKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Filter => "filter",
            Self::Delay => "delay",
            Self::Comb => "comb",
            Self::Allpass => "allpass",
            Self::Gain => "gain",
            Self::Shaper => "shaper",
            Self::Crusher => "crusher",
            Self::Width => "width",
            Self::Pan => "pan",
            Self::Dynamics => "dynamics",
            Self::Ring => "ring",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Filter => "Filter",
            Self::Delay => "Delay line",
            Self::Comb => "Comb",
            Self::Allpass => "All-pass",
            Self::Gain => "Gain",
            Self::Shaper => "Waveshaper",
            Self::Crusher => "Bit and rate crusher",
            Self::Width => "Stereo width",
            Self::Pan => "Pan",
            Self::Dynamics => "Compressor",
            Self::Ring => "Ring modulator",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        BLOCK_KINDS.into_iter().find(|kind| kind.id() == id)
    }

    pub fn params(self) -> &'static [ParamSpec] {
        match self {
            Self::Filter => {
                const P: &[ParamSpec] = &[
                    choice("type", "Type", FILTER_TYPES, 0),
                    param("frequency", "Frequency", 20.0, 20000.0, 1000.0, 1.0, "Hz"),
                    param("q", "Resonance (Q)", 0.1, 20.0, 0.707, 0.01, ""),
                    param("gain", "Gain", -24.0, 24.0, 0.0, 0.1, "dB"),
                    param("mix", "Mix", 0.0, 1.0, 1.0, 0.01, ""),
                ];
                P
            }
            Self::Delay => {
                const P: &[ParamSpec] = &[
                    param("time", "Time", 0.1, 2000.0, 250.0, 0.1, "ms"),
                    param("feedback", "Feedback", 0.0, 0.98, 0.3, 0.01, ""),
                    param("damping", "Damping", 0.0, 1.0, 0.2, 0.01, ""),
                    param("mix", "Mix", 0.0, 1.0, 0.5, 0.01, ""),
                ];
                P
            }
            Self::Comb => {
                const P: &[ParamSpec] = &[
                    param("time", "Time", 1.0, 100.0, 30.0, 0.1, "ms"),
                    param("feedback", "Feedback", 0.0, 0.99, 0.8, 0.01, ""),
                    param("damping", "Damping", 0.0, 1.0, 0.3, 0.01, ""),
                    param("mix", "Mix", 0.0, 1.0, 1.0, 0.01, ""),
                ];
                P
            }
            Self::Allpass => {
                const P: &[ParamSpec] = &[
                    param("time", "Time", 0.1, 50.0, 5.0, 0.1, "ms"),
                    param("gain", "Diffusion", -0.95, 0.95, 0.5, 0.01, ""),
                ];
                P
            }
            Self::Gain => {
                const P: &[ParamSpec] = &[param("gain", "Gain", -60.0, 24.0, 0.0, 0.1, "dB")];
                P
            }
            Self::Shaper => {
                const P: &[ParamSpec] = &[
                    choice("curve", "Curve", SHAPER_CURVES, 0),
                    param("drive", "Drive", 0.0, 40.0, 12.0, 0.1, "dB"),
                    param("mix", "Mix", 0.0, 1.0, 1.0, 0.01, ""),
                ];
                P
            }
            Self::Crusher => {
                const P: &[ParamSpec] = &[
                    param("bits", "Bits", 1.0, 16.0, 8.0, 1.0, ""),
                    param("rate", "Sample rate", 500.0, 48000.0, 11025.0, 1.0, "Hz"),
                    param("mix", "Mix", 0.0, 1.0, 1.0, 0.01, ""),
                ];
                P
            }
            Self::Width => {
                const P: &[ParamSpec] = &[param("width", "Width", 0.0, 2.0, 1.5, 0.01, "×")];
                P
            }
            Self::Pan => {
                const P: &[ParamSpec] = &[param("pan", "Pan", -1.0, 1.0, 0.0, 0.01, "")];
                P
            }
            Self::Dynamics => {
                const P: &[ParamSpec] = &[
                    param("threshold", "Threshold", -60.0, 0.0, -18.0, 0.1, "dB"),
                    param("ratio", "Ratio", 1.0, 20.0, 4.0, 0.1, ":1"),
                    param("attack", "Attack", 0.1, 200.0, 5.0, 0.1, "ms"),
                    param("release", "Release", 5.0, 2000.0, 120.0, 1.0, "ms"),
                    param("makeup", "Makeup gain", 0.0, 24.0, 3.0, 0.1, "dB"),
                ];
                P
            }
            Self::Ring => {
                const P: &[ParamSpec] = &[
                    param("frequency", "Frequency", 1.0, 5000.0, 440.0, 0.1, "Hz"),
                    param("mix", "Mix", 0.0, 1.0, 0.5, 0.01, ""),
                ];
                P
            }
        }
    }
}

pub const MODULATOR_KINDS: [ModulatorKind; 2] = [ModulatorKind::Lfo, ModulatorKind::Envelope];

/// Sources that move block settings over time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulatorKind {
    Lfo,
    Envelope,
}

impl ModulatorKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Lfo => "lfo",
            Self::Envelope => "envelope",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Lfo => "LFO",
            Self::Envelope => "Envelope follower",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [Self::Lfo, Self::Envelope].into_iter().find(|kind| kind.id() == id)
    }

    pub fn params(self) -> &'static [ParamSpec] {
        match self {
            Self::Lfo => {
                const P: &[ParamSpec] = &[
                    choice("shape", "Shape", LFO_SHAPES, 0),
                    param("rate", "Rate", 0.01, 20.0, 0.5, 0.01, "Hz"),
                    param("stereo", "Stereo phase", 0.0, 1.0, 0.0, 0.01, ""),
                ];
                P
            }
            Self::Envelope => {
                const P: &[ParamSpec] = &[
                    param("attack", "Attack", 0.1, 500.0, 10.0, 0.1, "ms"),
                    param("release", "Release", 5.0, 2000.0, 200.0, 1.0, "ms"),
                    param("sensitivity", "Sensitivity", -24.0, 24.0, 0.0, 0.1, "dB"),
                ];
                P
            }
        }
    }
}

fn values(specs: &[ParamSpec], json: Option<&Value>) -> BTreeMap<String, f32> {
    specs
        .iter()
        .map(|spec| {
            let value = json
                .and_then(|params| params.get(spec.id))
                .and_then(Value::as_f64)
                .map(|value| value as f32)
                .filter(|value| value.is_finite())
                .map_or(spec.default, |value| spec.clamp(value));
            (spec.id.to_owned(), value)
        })
        .collect()
}

fn values_json(values: &BTreeMap<String, f32>) -> Value {
    Value::Object(values.iter().map(|(id, value)| (id.clone(), json!(value))).collect())
}

/// A setting moved by a modulator: `depth` of the setting's range per unit
/// of the modulator (−1 … 1 for an LFO, 0 … 1 for an envelope).
#[derive(Clone, Debug, PartialEq)]
pub struct Modulation {
    pub param: String,
    pub source: usize,
    pub depth: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub kind: BlockKind,
    pub enabled: bool,
    pub params: BTreeMap<String, f32>,
    pub modulations: Vec<Modulation>,
}

impl Block {
    pub fn new(kind: BlockKind) -> Self {
        Self { kind, enabled: true, params: values(kind.params(), None), modulations: Vec::new() }
    }

    pub fn with(mut self, settings: &[(&str, f32)]) -> Self {
        for (id, value) in settings {
            self.params.insert((*id).to_owned(), *value);
        }
        self
    }

    pub fn modulated(mut self, param: &str, source: usize, depth: f32) -> Self {
        self.modulations.push(Modulation { param: param.to_owned(), source, depth });
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Modulator {
    pub kind: ModulatorKind,
    pub params: BTreeMap<String, f32>,
}

impl Modulator {
    pub fn new(kind: ModulatorKind) -> Self {
        Self { kind, params: values(kind.params(), None) }
    }

    pub fn with(mut self, settings: &[(&str, f32)]) -> Self {
        for (id, value) in settings {
            self.params.insert((*id).to_owned(), *value);
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    /// Linear level the branch is summed at.
    pub level: f32,
    pub stages: Vec<Stage>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    Block(Block),
    Parallel(Vec<Branch>),
    Feedback { amount: f32, stages: Vec<Stage> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub name: String,
    pub modulators: Vec<Modulator>,
    pub stages: Vec<Stage>,
}

/// How deep stages may nest, and how many blocks a patch may hold.
const MAX_DEPTH: usize = 6;
const MAX_BLOCKS: usize = 64;

impl Patch {
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "modulators": self.modulators.iter().map(|m| json!({"kind": m.kind.id(), "params": values_json(&m.params)})).collect::<Vec<_>>(),
            "stages": stages_json(&self.stages),
        })
    }

    pub fn from_json(value: &Value) -> Option<Self> {
        let name: String = value.get("name")?.as_str()?.chars().filter(|c| !c.is_control()).take(60).collect();
        if name.trim().is_empty() {
            return None;
        }
        let modulators: Vec<Modulator> = value
            .get("modulators")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let kind = ModulatorKind::from_id(item.get("kind")?.as_str()?)?;
                        Some(Modulator { kind, params: values(kind.params(), item.get("params")) })
                    })
                    .take(8)
                    .collect()
            })
            .unwrap_or_default();
        let mut blocks = 0;
        let stages = stages_from_json(value.get("stages")?, 0, &mut blocks, modulators.len());
        Some(Self { name, modulators, stages })
    }

    /// Every block, depth first.
    pub fn block_count(&self) -> usize {
        fn count(stages: &[Stage]) -> usize {
            stages
                .iter()
                .map(|stage| match stage {
                    Stage::Block(_) => 1,
                    Stage::Parallel(branches) => branches.iter().map(|b| count(&b.stages)).sum(),
                    Stage::Feedback { stages, .. } => count(stages),
                })
                .sum()
        }
        count(&self.stages)
    }
}

fn stages_json(stages: &[Stage]) -> Value {
    Value::Array(
        stages
            .iter()
            .map(|stage| match stage {
                Stage::Block(block) => json!({
                    "block": block.kind.id(),
                    "enabled": block.enabled,
                    "params": values_json(&block.params),
                    "modulations": block.modulations.iter().map(|m| json!({"param": m.param, "source": m.source, "depth": m.depth})).collect::<Vec<_>>(),
                }),
                Stage::Parallel(branches) => json!({
                    "parallel": branches.iter().map(|branch| json!({"level": branch.level, "stages": stages_json(&branch.stages)})).collect::<Vec<_>>(),
                }),
                Stage::Feedback { amount, stages } => json!({"feedback": amount, "stages": stages_json(stages)}),
            })
            .collect(),
    )
}

fn stages_from_json(value: &Value, depth: usize, blocks: &mut usize, modulators: usize) -> Vec<Stage> {
    let Some(items) = value.as_array() else { return Vec::new() };
    if depth > MAX_DEPTH {
        return Vec::new();
    }
    items
        .iter()
        .filter_map(|item| {
            if let Some(kind) = item.get("block").and_then(Value::as_str).and_then(BlockKind::from_id) {
                if *blocks >= MAX_BLOCKS {
                    return None;
                }
                *blocks += 1;
                let modulations = item
                    .get("modulations")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|m| {
                                let param = m.get("param")?.as_str()?;
                                kind.params().iter().find(|spec| spec.id == param && spec.choices.is_empty())?;
                                let source = usize::try_from(m.get("source")?.as_u64()?).ok().filter(|s| *s < modulators)?;
                                let depth = (m.get("depth")?.as_f64()? as f32).clamp(-1.0, 1.0);
                                Some(Modulation { param: param.to_owned(), source, depth })
                            })
                            .take(8)
                            .collect()
                    })
                    .unwrap_or_default();
                return Some(Stage::Block(Block {
                    kind,
                    enabled: item.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                    params: values(kind.params(), item.get("params")),
                    modulations,
                }));
            }
            if let Some(branches) = item.get("parallel").and_then(Value::as_array) {
                return Some(Stage::Parallel(
                    branches
                        .iter()
                        .take(8)
                        .map(|branch| Branch {
                            level: branch.get("level").and_then(Value::as_f64).map_or(1.0, |l| (l as f32).clamp(0.0, 2.0)),
                            stages: branch.get("stages").map_or_else(Vec::new, |s| stages_from_json(s, depth + 1, blocks, modulators)),
                        })
                        .collect(),
                ));
            }
            if let Some(amount) = item.get("feedback").and_then(Value::as_f64) {
                return Some(Stage::Feedback {
                    amount: (amount as f32).clamp(0.0, 0.98),
                    stages: item.get("stages").map_or_else(Vec::new, |s| stages_from_json(s, depth + 1, blocks, modulators)),
                });
            }
            None
        })
        .collect()
}

/// The building-block catalog for settings screens.
pub fn catalog() -> Value {
    let spec_json = |spec: &ParamSpec| {
        json!({"id": spec.id, "label": spec.label, "min": spec.min, "max": spec.max, "default": spec.default,
               "step": spec.step, "unit": spec.unit, "choices": spec.choices})
    };
    json!({
        "blocks": BLOCK_KINDS.iter().map(|kind| json!({"kind": kind.id(), "label": kind.label(), "params": kind.params().iter().map(spec_json).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "modulators": MODULATOR_KINDS.iter().map(|kind| json!({"kind": kind.id(), "label": kind.label(), "params": kind.params().iter().map(spec_json).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "templates": templates().iter().map(Patch::to_json).collect::<Vec<_>>(),
    })
}

/// The eight built-in effects rebuilt from blocks, to open and change.
pub fn templates() -> Vec<Patch> {
    use BlockKind::*;
    let block = Block::new;
    let stage = Stage::Block;
    let branch = |level: f32, stages: Vec<Stage>| Branch { level, stages };
    let dry = || branch(1.0, Vec::new());
    vec![
        Patch {
            name: "Reverb (blocks)".into(),
            modulators: vec![],
            stages: vec![Stage::Parallel(vec![
                dry(),
                branch(
                    0.35,
                    vec![
                        Stage::Parallel(
                            [25.3, 26.9, 28.9, 30.7, 32.2, 33.8]
                                .iter()
                                .map(|time| branch(0.3, vec![stage(block(Comb).with(&[("time", *time), ("feedback", 0.84), ("damping", 0.25)]))]))
                                .collect(),
                        ),
                        stage(block(Allpass).with(&[("time", 12.6), ("gain", 0.5)])),
                        stage(block(Allpass).with(&[("time", 10.0), ("gain", 0.5)])),
                        stage(block(Allpass).with(&[("time", 7.7), ("gain", 0.5)])),
                        stage(block(Width).with(&[("width", 1.3)])),
                    ],
                ),
            ])],
        },
        Patch { name: "Widener (blocks)".into(), modulators: vec![], stages: vec![stage(block(Width).with(&[("width", 1.5)]))] },
        Patch {
            name: "Echo (blocks)".into(),
            modulators: vec![],
            stages: vec![stage(block(Delay).with(&[("time", 350.0), ("feedback", 0.35), ("damping", 0.3), ("mix", 0.3)]))],
        },
        Patch {
            name: "Chorus (blocks)".into(),
            modulators: vec![Modulator::new(ModulatorKind::Lfo).with(&[("rate", 0.8), ("stereo", 0.25)])],
            stages: vec![stage(block(Delay).with(&[("time", 12.0), ("feedback", 0.0), ("damping", 0.0), ("mix", 0.4)]).modulated("time", 0, 0.0015))],
        },
        Patch {
            name: "Warmth (blocks)".into(),
            modulators: vec![],
            stages: vec![
                stage(block(Filter).with(&[("type", 5.0), ("frequency", 120.0), ("gain", 4.0)])),
                stage(block(Filter).with(&[("type", 0.0), ("frequency", 9000.0)])),
            ],
        },
        Patch {
            name: "Bitcrusher (blocks)".into(),
            modulators: vec![],
            stages: vec![stage(block(Crusher).with(&[("bits", 8.0), ("rate", 11025.0)]))],
        },
        Patch {
            name: "Distortion (blocks)".into(),
            modulators: vec![],
            stages: vec![
                stage(block(Shaper).with(&[("curve", 0.0), ("drive", 12.0), ("mix", 0.6)])),
                stage(block(Filter).with(&[("type", 0.0), ("frequency", 6000.0)])),
            ],
        },
        Patch {
            name: "Compressor (blocks)".into(),
            modulators: vec![],
            stages: vec![stage(block(Dynamics)), stage(block(Shaper).with(&[("curve", 0.0), ("drive", 0.0)]))],
        },
        Patch {
            name: "Auto-wah".into(),
            modulators: vec![Modulator::new(ModulatorKind::Envelope).with(&[("attack", 5.0), ("release", 150.0), ("sensitivity", 12.0)])],
            stages: vec![stage(block(Filter).with(&[("type", 2.0), ("frequency", 400.0), ("q", 4.0)]).modulated("frequency", 0, 0.15))],
        },
        Patch {
            name: "Tremolo".into(),
            modulators: vec![Modulator::new(ModulatorKind::Lfo).with(&[("rate", 5.0)])],
            stages: vec![stage(block(Gain).with(&[("gain", -6.0)]).modulated("gain", 0, 0.07))],
        },
    ]
}

// ---------------------------------------------------------------------------
// Running a patch

/// Modulator values for this sample, per side.
type ModValues = Vec<[f32; 2]>;

struct ModState {
    kind: ModulatorKind,
    shape: usize,
    step: f32,
    stereo: f32,
    phase: f32,
    random: [f32; 2],
    seed: u32,
    attack: f32,
    release: f32,
    sensitivity: f32,
    envelope: f32,
}

impl ModState {
    fn new(modulator: &Modulator, rate: f32) -> Self {
        let get = |id: &str| modulator.params.get(id).copied().unwrap_or_default();
        let coefficient = |ms: f32| (-1.0 / (ms.max(0.01) / 1000.0 * rate)).exp();
        Self {
            kind: modulator.kind,
            shape: get("shape") as usize,
            step: get("rate") / rate,
            stereo: get("stereo"),
            phase: 0.0,
            random: [0.0; 2],
            seed: 0x1234_5678,
            attack: coefficient(get("attack")),
            release: coefficient(get("release")),
            sensitivity: 10f32.powf(get("sensitivity") / 20.0),
            envelope: 0.0,
        }
    }

    fn next(&mut self, input: f32) -> [f32; 2] {
        match self.kind {
            ModulatorKind::Lfo => {
                let previous = self.phase;
                self.phase = (self.phase + self.step).fract();
                let wave = |phase: f32, random: f32| match self.shape {
                    1 => 1.0 - 4.0 * (phase - 0.5).abs(),
                    2 => {
                        if phase < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    3 => 2.0 * phase - 1.0,
                    4 => random,
                    _ => (TAU * phase).sin(),
                };
                if self.shape == 4 && self.phase < previous {
                    for side in 0..2 {
                        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        self.random[side] = (self.seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0;
                    }
                }
                [wave(self.phase, self.random[0]), wave((self.phase + self.stereo).fract(), self.random[1])]
            }
            ModulatorKind::Envelope => {
                let level = (input.abs() * self.sensitivity).min(1.0);
                let coefficient = if level > self.envelope { self.attack } else { self.release };
                self.envelope = level + coefficient * (self.envelope - level);
                [self.envelope; 2]
            }
        }
    }
}

/// A block's settings for one side this sample, after modulation.
fn effective(block: &Block, mods: &ModValues, side: usize) -> BTreeMap<String, f32> {
    let mut params = block.params.clone();
    for modulation in &block.modulations {
        let Some(spec) = block.kind.params().iter().find(|spec| spec.id == modulation.param) else { continue };
        let Some(value) = mods.get(modulation.source) else { continue };
        let base = params.get(&modulation.param).copied().unwrap_or(spec.default);
        params.insert(modulation.param.clone(), spec.clamp(base + modulation.depth * value[side] * (spec.max - spec.min)));
    }
    params
}

struct BlockState {
    block: Block,
    rate: f32,
    // Per side.
    filters: [Biquad; 2],
    filter_key: [(usize, f32, f32, f32); 2],
    lines: [DelayLine; 2],
    damp_state: [f32; 2],
    held: [f32; 2],
    hold_phase: f32,
    envelope_db: f32,
    ring_phase: f32,
    countdown: u32,
    params: [BTreeMap<String, f32>; 2],
}

impl BlockState {
    fn new(block: &Block, rate: f32) -> Self {
        let longest = block
            .kind
            .params()
            .iter()
            .find(|spec| spec.id == "time")
            .map_or(2, |spec| (spec.max / 1000.0 * rate) as usize + 4);
        let params = block.params.clone();
        Self {
            block: block.clone(),
            rate,
            filters: [Biquad::default(); 2],
            filter_key: [(usize::MAX, 0.0, 0.0, 0.0); 2],
            lines: [DelayLine::new(longest), DelayLine::new(longest)],
            damp_state: [0.0; 2],
            held: [0.0; 2],
            hold_phase: 1.0,
            envelope_db: -120.0,
            ring_phase: 0.0,
            countdown: 0,
            params: [params.clone(), params],
        }
    }

    fn process(&mut self, input: [f32; 2], mods: &ModValues) -> [f32; 2] {
        if !self.block.enabled {
            return input;
        }
        // Modulated settings are refreshed every 16 samples.
        if !self.block.modulations.is_empty() {
            if self.countdown == 0 {
                self.params = [effective(&self.block, mods, 0), effective(&self.block, mods, 1)];
                self.countdown = 16;
            }
            self.countdown -= 1;
        }
        let rate = self.rate;
        let get = |params: &BTreeMap<String, f32>, id: &str| params.get(id).copied().unwrap_or_default();
        let mut output = input;
        match self.block.kind {
            BlockKind::Filter => {
                for side in 0..2 {
                    let p = &self.params[side];
                    let key = (get(p, "type") as usize, get(p, "frequency"), get(p, "q"), get(p, "gain"));
                    if key != self.filter_key[side] {
                        let state = (self.filters[side].z1, self.filters[side].z2);
                        self.filters[side] = Biquad::design(key.0, key.1, key.2, key.3, rate);
                        (self.filters[side].z1, self.filters[side].z2) = state;
                        self.filter_key[side] = key;
                    }
                    let wet = self.filters[side].process(input[side]);
                    output[side] = mix(input[side], wet, get(p, "mix"));
                }
            }
            BlockKind::Delay | BlockKind::Comb => {
                for side in 0..2 {
                    let p = &self.params[side];
                    let delayed = self.lines[side].read(get(p, "time") / 1000.0 * rate);
                    let damping = get(p, "damping");
                    self.damp_state[side] = delayed * (1.0 - damping) + self.damp_state[side] * damping;
                    let feedback = get(p, "feedback");
                    let fed = (input[side] + self.damp_state[side] * feedback).clamp(-4.0, 4.0);
                    self.lines[side].push(fed);
                    output[side] = mix(input[side], delayed, get(p, "mix"));
                }
            }
            BlockKind::Allpass => {
                for side in 0..2 {
                    let p = &self.params[side];
                    let gain = get(p, "gain");
                    let delayed = self.lines[side].read(get(p, "time") / 1000.0 * rate);
                    let fed = input[side] + delayed * gain;
                    self.lines[side].push(fed);
                    output[side] = delayed - fed * gain;
                }
            }
            BlockKind::Gain => {
                for side in 0..2 {
                    output[side] = input[side] * 10f32.powf(get(&self.params[side], "gain") / 20.0);
                }
            }
            BlockKind::Shaper => {
                for side in 0..2 {
                    let p = &self.params[side];
                    let drive = 10f32.powf(get(p, "drive") / 20.0);
                    let x = input[side] * drive;
                    let shaped = match get(p, "curve") as usize {
                        1 => x.clamp(-1.0, 1.0),
                        2 => {
                            // Fold back into range, like a wavefolder.
                            let folded = (x + 1.0).rem_euclid(4.0);
                            if folded > 2.0 { 3.0 - folded } else { folded - 1.0 }
                        }
                        3 => (x * std::f32::consts::FRAC_PI_2).sin(),
                        4 => {
                            if x >= 0.0 {
                                x.tanh()
                            } else {
                                (x * 0.5).tanh() * 1.5
                            }
                        }
                        _ => x.tanh(),
                    };
                    // Keep full-scale input at full scale as the drive rises.
                    let makeup = 1.0 / drive.tanh().max(0.05);
                    output[side] = mix(input[side], shaped * makeup.min(1.0), get(p, "mix"));
                }
            }
            BlockKind::Crusher => {
                let p = &self.params[0];
                self.hold_phase += (get(p, "rate") / rate).min(1.0);
                if self.hold_phase >= 1.0 {
                    self.hold_phase -= 1.0;
                    let levels = 2f32.powf(get(p, "bits") - 1.0).max(1.0);
                    self.held = input.map(|value| (value.clamp(-1.0, 1.0) * levels).round() / levels);
                }
                let amount = get(p, "mix");
                output = [mix(input[0], self.held[0], amount), mix(input[1], self.held[1], amount)];
            }
            BlockKind::Width => {
                let width = get(&self.params[0], "width");
                let middle = (input[0] + input[1]) * 0.5;
                let side = (input[0] - input[1]) * 0.5 * width;
                let gain = 1.0 / (1.0 + (width - 1.0).max(0.0) * 0.25);
                output = [(middle + side) * gain, (middle - side) * gain];
            }
            BlockKind::Pan => {
                let pan = get(&self.params[0], "pan");
                let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
                let middle = (input[0] + input[1]) * 0.5;
                let side = (input[0] - input[1]) * 0.5;
                output = [(middle + side) * angle.cos() * std::f32::consts::SQRT_2, (middle - side) * angle.sin() * std::f32::consts::SQRT_2];
            }
            BlockKind::Dynamics => {
                let p = &self.params[0];
                let peak = input[0].abs().max(input[1].abs()).max(1e-6);
                let level = 20.0 * peak.log10();
                let ms = if level > self.envelope_db { get(p, "attack") } else { get(p, "release") };
                let coefficient = (-1.0 / (ms / 1000.0 * rate)).exp();
                self.envelope_db = level + coefficient * (self.envelope_db - level);
                let over = self.envelope_db - get(p, "threshold");
                let reduction = if over > 0.0 { over * (1.0 - 1.0 / get(p, "ratio")) } else { 0.0 };
                let gain = 10f32.powf((get(p, "makeup") - reduction) / 20.0);
                output = input.map(|value| value * gain);
            }
            BlockKind::Ring => {
                let p = &self.params[0];
                self.ring_phase = (self.ring_phase + get(p, "frequency") / rate).fract();
                let carrier = (TAU * self.ring_phase).sin();
                let amount = get(p, "mix");
                output = input.map(|value| mix(value, value * carrier, amount));
            }
        }
        output
    }
}

fn mix(dry: f32, wet: f32, amount: f32) -> f32 {
    dry + (wet - dry) * amount
}

enum StageState {
    Block(Box<BlockState>),
    Parallel(Vec<(f32, Vec<StageState>)>),
    Feedback { amount: f32, stages: Vec<StageState>, last: [f32; 2] },
}

fn build_stages(stages: &[Stage], rate: f32) -> Vec<StageState> {
    stages
        .iter()
        .map(|stage| match stage {
            Stage::Block(block) => StageState::Block(Box::new(BlockState::new(block, rate))),
            Stage::Parallel(branches) => {
                StageState::Parallel(branches.iter().map(|branch| (branch.level, build_stages(&branch.stages, rate))).collect())
            }
            Stage::Feedback { amount, stages } => StageState::Feedback { amount: *amount, stages: build_stages(stages, rate), last: [0.0; 2] },
        })
        .collect()
}

fn run(stages: &mut [StageState], input: [f32; 2], mods: &ModValues) -> [f32; 2] {
    let mut signal = input;
    for stage in stages {
        signal = match stage {
            StageState::Block(block) => block.process(signal, mods),
            StageState::Parallel(branches) => {
                let mut sum = [0.0; 2];
                for (level, branch) in branches.iter_mut() {
                    let out = run(branch, signal, mods);
                    sum[0] += out[0] * *level;
                    sum[1] += out[1] * *level;
                }
                sum
            }
            StageState::Feedback { amount, stages, last } => {
                // A loop with no delay of its own still waits one sample,
                // and is kept from running away.
                let fed = [signal[0] + last[0] * *amount, signal[1] + last[1] * *amount];
                let out = run(stages, fed, mods);
                *last = out.map(|value| value.clamp(-2.0, 2.0).tanh());
                out
            }
        };
    }
    signal
}

pub(crate) struct PatchEffect {
    modulators: Vec<ModState>,
    stages: Vec<StageState>,
    mods: ModValues,
    mix: f32,
}

impl PatchEffect {
    pub(crate) fn new(patch: &Patch, rate: f32, mix: f32) -> Self {
        Self {
            modulators: patch.modulators.iter().map(|m| ModState::new(m, rate)).collect(),
            stages: build_stages(&patch.stages, rate),
            mods: vec![[0.0; 2]; patch.modulators.len()],
            mix,
        }
    }
}

impl Effect for PatchEffect {
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let level = (left.abs() + right.abs()) * 0.5;
        for (state, value) in self.modulators.iter_mut().zip(&mut self.mods) {
            *value = state.next(level);
        }
        let out = run(&mut self.stages, [left, right], &self.mods);
        let safe = |value: f32| if value.is_finite() { value } else { 0.0 };
        (mix(left, safe(out[0]), self.mix), mix(right, safe(out[1]), self.mix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_round_trip_through_json() {
        for patch in templates() {
            let parsed = Patch::from_json(&patch.to_json()).unwrap();
            assert_eq!(parsed, patch, "{}", patch.name);
        }
    }

    #[test]
    fn every_template_runs_finite_and_changes_the_sound() {
        for patch in templates() {
            let mut effect = PatchEffect::new(&patch, 48_000.0, 1.0);
            let mut changed = false;
            for i in 0..48_000 {
                let x = ((i * 7919 % 2003) as f32 / 1001.5 - 1.0) * 0.5;
                let (l, r) = effect.process(x, -x * 0.5);
                assert!(l.is_finite() && r.is_finite(), "{}", patch.name);
                changed |= (l - x).abs() > 1e-4 || (r + x * 0.5).abs() > 1e-4;
            }
            assert!(changed, "{} changed nothing", patch.name);
        }
    }

    #[test]
    fn bad_patches_are_trimmed_not_trusted() {
        let text = r#"{"name":"x","modulators":[{"kind":"lfo","params":{"rate":999}}],
            "stages":[{"block":"filter","params":{"frequency":-5},"modulations":[{"param":"frequency","source":7,"depth":3}]},
                      {"block":"nope"},{"feedback":5,"stages":[{"block":"delay"}]}]}"#;
        let patch = Patch::from_json(&serde_json::from_str(text).unwrap()).unwrap();
        assert_eq!(patch.modulators[0].params["rate"], 20.0);
        let Stage::Block(filter) = &patch.stages[0] else { panic!() };
        assert_eq!(filter.params["frequency"], 20.0);
        assert!(filter.modulations.is_empty());
        assert_eq!(patch.stages.len(), 2);
        let Stage::Feedback { amount, .. } = patch.stages[1] else { panic!() };
        assert_eq!(amount, 0.98);
    }
}
