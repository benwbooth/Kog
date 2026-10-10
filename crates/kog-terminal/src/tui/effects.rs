//! The effects chain in the terminal: a full-screen list of effects and their
//! settings, edited with the keyboard and applied as it changes. Custom
//! effects built from blocks are listed below the chain and open in a patch
//! editor on the same screen.
use super::{Key, Surface, paint};
use kog_core::effects::{EFFECT_KINDS, EffectKind, EffectSlot, EffectsSettings, MAX_EFFECTS, MAX_PATCHES, ParamSpec, presets};
use kog_core::patch::{BLOCK_KINDS, Block, Branch, MODULATOR_KINDS, Modulation, Modulator, Patch, Stage, templates};

/// A step on the way to a stage: a stage in a list, or a path of the
/// parallel stage reached so far.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Stage(usize),
    Branch(usize),
}

/// One selectable line of the chain screen.
#[derive(Clone, Copy, PartialEq)]
enum Line {
    Enabled,
    Preset,
    Slot(usize),
    Param(usize, usize),
    Patch(usize),
}

/// One selectable line of the patch editor.
#[derive(Clone, PartialEq)]
enum PatchLine {
    Modulator(usize),
    ModulatorParam(usize, usize),
    /// A stage at this path.
    Stage(Vec<Step>),
    /// A block setting: block path, setting index.
    Param(Vec<Step>, usize),
    /// A path of a parallel stage: the stage's path plus the branch.
    Branch(Vec<Step>),
}

/// What a key that picks from a numbered list will do.
#[derive(Clone, Copy, PartialEq)]
enum Picking {
    Effect,
    Custom,
    Template,
    Block,
}

#[derive(Default)]
pub(super) struct Effects {
    pub open: bool,
    settings: EffectsSettings,
    cursor: usize,
    picking: Option<Picking>,
    /// The patch being edited, by index into the saved patches.
    editing: Option<usize>,
    patch_cursor: usize,
}

const PICK_KEYS: &str = "123456789abcdefghijk";

fn pick_index(key: Key) -> Option<usize> {
    let Key::Char(c) = key else { return None };
    PICK_KEYS.find(c)
}

/// The list of stages a path's last step sits in, and the index within it.
fn container<'a>(stages: &'a mut Vec<Stage>, path: &[Step]) -> Option<(&'a mut Vec<Stage>, usize)> {
    let (last, parents) = path.split_last()?;
    let Step::Stage(index) = *last else { return None };
    let mut list = stages;
    let mut steps = parents.iter().peekable();
    while let Some(step) = steps.next() {
        let Step::Stage(at) = *step else { return None };
        list = match list.get_mut(at)? {
            Stage::Parallel(branches) => {
                let Some(Step::Branch(branch)) = steps.next().copied() else { return None };
                &mut branches.get_mut(branch)?.stages
            }
            Stage::Feedback { stages, .. } => stages,
            Stage::Block(_) => return None,
        };
    }
    Some((list, index))
}

fn stage_at<'a>(stages: &'a mut Vec<Stage>, path: &[Step]) -> Option<&'a mut Stage> {
    let (list, index) = container(stages, path)?;
    list.get_mut(index)
}

fn rows(stages: &[Stage], path: &mut Vec<Step>, out: &mut Vec<PatchLine>) {
    for (index, stage) in stages.iter().enumerate() {
        path.push(Step::Stage(index));
        out.push(PatchLine::Stage(path.clone()));
        match stage {
            Stage::Block(block) => {
                out.extend((0..block.kind.params().len()).map(|param| PatchLine::Param(path.clone(), param)));
            }
            Stage::Parallel(branches) => {
                for (branch, inner) in branches.iter().enumerate() {
                    path.push(Step::Branch(branch));
                    out.push(PatchLine::Branch(path.clone()));
                    rows(&inner.stages, path, out);
                    path.pop();
                }
            }
            Stage::Feedback { stages, .. } => rows(stages, path, out),
        }
        path.pop();
    }
}

fn step_value(spec: &ParamSpec, value: f32, direction: f32) -> f32 {
    if !spec.choices.is_empty() {
        let count = spec.choices.len() as f32;
        return (value + direction).rem_euclid(count);
    }
    // Fifty steps across the range, at least one unit step.
    let step = ((spec.max - spec.min) / 50.0).max(spec.step);
    let value = (value + step * direction).clamp(spec.min, spec.max);
    (value / spec.step).round() * spec.step
}

fn shown(spec: &ParamSpec, value: f32) -> String {
    if !spec.choices.is_empty() {
        return spec.choices.get(value as usize).copied().unwrap_or("?").to_owned();
    }
    if spec.id == "ping_pong" {
        return if value >= 0.5 { "On" } else { "Off" }.to_owned();
    }
    let digits = if spec.step >= 1.0 { 0 } else if spec.step >= 0.1 { 1 } else { 2 };
    format!("{value:.digits$} {}", spec.unit).trim_end().to_owned()
}

fn bar(spec: &ParamSpec, value: f32) -> String {
    let filled = ((((value - spec.min) / (spec.max - spec.min)).clamp(0.0, 1.0)) * 20.0).round() as usize;
    format!("{}{}", "█".repeat(filled), "·".repeat(20 - filled))
}

impl Effects {
    pub fn show(&mut self, settings: EffectsSettings) {
        self.settings = settings;
        self.open = true;
        self.picking = None;
        self.editing = None;
        self.cursor = self.cursor.min(self.lines().len().saturating_sub(1));
    }

    fn lines(&self) -> Vec<Line> {
        let mut lines = vec![Line::Enabled, Line::Preset];
        for (slot, effect) in self.settings.chain.iter().enumerate() {
            lines.push(Line::Slot(slot));
            lines.extend((0..effect.kind.params().len()).map(|param| Line::Param(slot, param)));
        }
        lines.extend((0..self.settings.patches.len()).map(Line::Patch));
        lines
    }

    fn current(&self) -> Line {
        let lines = self.lines();
        lines[self.cursor.min(lines.len() - 1)]
    }

    fn select(&mut self, line: Line) {
        if let Some(at) = self.lines().iter().position(|candidate| *candidate == line) {
            self.cursor = at;
        }
    }

    fn changed(&mut self) -> Option<EffectsSettings> {
        Some(self.settings.clone())
    }

    fn free_name(&self, base: &str) -> String {
        let mut name = base.to_owned();
        let mut n = 2;
        while self.settings.patches.iter().any(|patch| patch.name == name) {
            name = format!("{base} {n}");
            n += 1;
        }
        name
    }

    /// Handle a key; returns the new settings when they changed.
    pub fn key(&mut self, key: Key) -> Option<EffectsSettings> {
        if let Some(picking) = self.picking.take() {
            return self.pick(picking, key);
        }
        if self.editing.is_some() {
            return self.patch_key(key);
        }
        let lines = self.lines().len();
        match key {
            Key::Esc | Key::Char('q') => self.open = false,
            Key::Up => self.cursor = self.cursor.saturating_sub(1),
            Key::Down => self.cursor = (self.cursor + 1).min(lines - 1),
            Key::Char('a') if self.settings.chain.len() < MAX_EFFECTS => self.picking = Some(Picking::Effect),
            Key::Char('c') if self.settings.chain.len() < MAX_EFFECTS && !self.settings.patches.is_empty() => {
                self.picking = Some(Picking::Custom)
            }
            Key::Char('n') if self.settings.patches.len() < MAX_PATCHES => self.picking = Some(Picking::Template),
            Key::Char('e') | Key::Enter => {
                let patch = match self.current() {
                    Line::Patch(patch) => Some(patch),
                    Line::Slot(slot) | Line::Param(slot, _) => {
                        let name = self.settings.chain[slot].patch.clone();
                        self.settings.patches.iter().position(|patch| Some(&patch.name) == name.as_ref())
                    }
                    _ => None,
                };
                if let Some(patch) = patch {
                    self.editing = Some(patch);
                    self.patch_cursor = 0;
                } else if key == Key::Enter {
                    return self.toggle();
                }
            }
            Key::Left | Key::Char('-') => return self.adjust(-1.0),
            Key::Right | Key::Char('+') | Key::Char('=') => return self.adjust(1.0),
            Key::Char(' ') => return self.toggle(),
            Key::Char('d') | Key::Delete => match self.current() {
                Line::Slot(slot) | Line::Param(slot, _) => {
                    self.settings.chain.remove(slot);
                    self.settings.preset_name.clear();
                    self.cursor = self.cursor.min(self.lines().len() - 1);
                    return self.changed();
                }
                Line::Patch(patch) => {
                    let name = self.settings.patches.remove(patch).name;
                    self.settings.chain.retain(|slot| slot.patch.as_deref() != Some(&name));
                    self.cursor = self.cursor.min(self.lines().len() - 1);
                    return self.changed();
                }
                _ => {}
            },
            Key::Char('[') | Key::Char(']') => {
                let (Line::Slot(slot) | Line::Param(slot, _)) = self.current() else { return None };
                let target = if key == Key::Char('[') { slot.checked_sub(1)? } else { slot + 1 };
                if target >= self.settings.chain.len() {
                    return None;
                }
                self.settings.chain.swap(slot, target);
                self.settings.preset_name.clear();
                self.select(Line::Slot(target));
                return self.changed();
            }
            _ => {}
        }
        None
    }

    fn pick(&mut self, picking: Picking, key: Key) -> Option<EffectsSettings> {
        let index = pick_index(key)?;
        match picking {
            Picking::Effect => {
                let kind = *EFFECT_KINDS.get(index)?;
                self.settings.chain.push(EffectSlot::new(kind));
            }
            Picking::Custom => {
                let name = self.settings.patches.get(index)?.name.clone();
                self.settings.chain.push(EffectSlot::custom(&name));
            }
            Picking::Template => {
                let mut patch = if index == 0 {
                    Patch { name: String::new(), modulators: Vec::new(), stages: Vec::new() }
                } else {
                    templates().get(index - 1)?.clone()
                };
                patch.name = self.free_name(if index == 0 { "My effect" } else { patch.name.trim_end_matches(" (blocks)") });
                self.settings.patches.push(patch);
                self.editing = Some(self.settings.patches.len() - 1);
                self.patch_cursor = 0;
                return self.changed();
            }
            Picking::Block => {
                let kind = *BLOCK_KINDS.get(index)?;
                return self.insert_stage(Stage::Block(Block::new(kind)));
            }
        }
        self.settings.enabled = true;
        self.settings.preset_name.clear();
        self.select(Line::Slot(self.settings.chain.len() - 1));
        self.changed()
    }

    fn toggle(&mut self) -> Option<EffectsSettings> {
        match self.current() {
            Line::Enabled => self.settings.enabled = !self.settings.enabled,
            Line::Slot(slot) => self.settings.chain[slot].enabled = !self.settings.chain[slot].enabled,
            Line::Preset => return self.adjust(1.0),
            Line::Param(..) | Line::Patch(_) => return None,
        }
        self.changed()
    }

    fn adjust(&mut self, direction: f32) -> Option<EffectsSettings> {
        match self.current() {
            Line::Enabled | Line::Slot(_) => return self.toggle(),
            Line::Patch(_) => return None,
            Line::Preset => {
                // Presets replace the chain; custom effects stay saved.
                let presets = presets();
                let at = presets.iter().position(|preset| preset.preset_name == self.settings.preset_name);
                let next = match (at, direction > 0.0) {
                    (Some(at), true) => (at + 1) % presets.len(),
                    (Some(at), false) => (at + presets.len() - 1) % presets.len(),
                    (None, _) => 0,
                };
                let patches = std::mem::take(&mut self.settings.patches);
                self.settings = presets[next].clone();
                self.settings.patches = patches;
            }
            Line::Param(slot, param) => {
                let effect = &mut self.settings.chain[slot];
                let spec = effect.kind.params()[param];
                let value = step_value(&spec, effect.get(spec.id), direction);
                effect.params.insert(spec.id.to_owned(), value);
                self.settings.preset_name.clear();
            }
        }
        self.changed()
    }

    // -----------------------------------------------------------------------
    // Patch editor

    fn patch(&mut self) -> &mut Patch {
        &mut self.settings.patches[self.editing.unwrap_or_default()]
    }

    fn patch_lines(&self) -> Vec<PatchLine> {
        let Some(patch) = self.editing.and_then(|index| self.settings.patches.get(index)) else { return Vec::new() };
        let mut out = Vec::new();
        for (index, modulator) in patch.modulators.iter().enumerate() {
            out.push(PatchLine::Modulator(index));
            out.extend((0..modulator.kind.params().len()).map(|param| PatchLine::ModulatorParam(index, param)));
        }
        rows(&patch.stages, &mut Vec::new(), &mut out);
        out
    }

    fn patch_current(&self) -> Option<PatchLine> {
        let lines = self.patch_lines();
        lines.get(self.patch_cursor.min(lines.len().saturating_sub(1))).cloned()
    }

    /// Add a stage after the selected one, or inside the selected parallel
    /// path or feedback loop.
    fn insert_stage(&mut self, stage: Stage) -> Option<EffectsSettings> {
        if self.patch().block_count() >= 64 {
            return None;
        }
        let current = self.patch_current();
        let patch = self.patch();
        match current {
            Some(PatchLine::Branch(path)) => {
                let (Some(Step::Branch(branch)), parent) = (path.last().copied(), &path[..path.len() - 1]) else { return None };
                let Stage::Parallel(branches) = stage_at(&mut patch.stages, parent)? else { return None };
                branches.get_mut(branch)?.stages.insert(0, stage);
            }
            Some(PatchLine::Stage(path)) | Some(PatchLine::Param(path, _)) => {
                if let Some(Stage::Feedback { stages, .. }) = stage_at(&mut patch.stages, &path) {
                    stages.push(stage);
                } else {
                    let (list, index) = container(&mut patch.stages, &path)?;
                    list.insert(index + 1, stage);
                }
            }
            _ => patch.stages.push(stage),
        }
        self.changed()
    }

    fn patch_key(&mut self, key: Key) -> Option<EffectsSettings> {
        let lines = self.patch_lines().len().max(1);
        match key {
            Key::Esc | Key::Char('q') => self.editing = None,
            Key::Up => self.patch_cursor = self.patch_cursor.saturating_sub(1),
            Key::Down => self.patch_cursor = (self.patch_cursor + 1).min(lines - 1),
            Key::Char('a') => self.picking = Some(Picking::Block),
            Key::Char('p') => {
                return self.insert_stage(Stage::Parallel(vec![
                    Branch { level: 1.0, stages: Vec::new() },
                    Branch { level: 1.0, stages: Vec::new() },
                ]));
            }
            Key::Char('f') => return self.insert_stage(Stage::Feedback { amount: 0.5, stages: Vec::new() }),
            Key::Char('l') | Key::Char('v') => {
                if self.patch().modulators.len() < 8 {
                    let kind = MODULATOR_KINDS[usize::from(key == Key::Char('v'))];
                    self.patch().modulators.push(Modulator::new(kind));
                    return self.changed();
                }
            }
            Key::Char(' ') => {
                if let Some(PatchLine::Stage(path)) = self.patch_current() {
                    if let Some(Stage::Block(block)) = stage_at(&mut self.patch().stages, &path) {
                        block.enabled = !block.enabled;
                        return self.changed();
                    }
                }
            }
            Key::Left | Key::Char('-') => return self.patch_adjust(-1.0),
            Key::Right | Key::Char('+') | Key::Char('=') => return self.patch_adjust(1.0),
            Key::Char('m') => return self.cycle_modulation(),
            Key::Char(',') | Key::Char('.') => return self.modulation_depth(if key == Key::Char('.') { 0.05 } else { -0.05 }),
            Key::Char('b') => {
                // Another path in the parallel group.
                let path = match self.patch_current()? {
                    PatchLine::Branch(path) => path[..path.len() - 1].to_vec(),
                    PatchLine::Stage(path) => path,
                    _ => return None,
                };
                if let Some(Stage::Parallel(branches)) = stage_at(&mut self.patch().stages, &path) {
                    if branches.len() < 8 {
                        branches.push(Branch { level: 1.0, stages: Vec::new() });
                        return self.changed();
                    }
                }
            }
            Key::Char('d') | Key::Delete => return self.patch_delete(),
            Key::Char('[') | Key::Char(']') => {
                let path = match self.patch_current()? {
                    PatchLine::Stage(path) | PatchLine::Param(path, _) => path,
                    _ => return None,
                };
                let up = key == Key::Char('[');
                let (list, index) = container(&mut self.patch().stages, &path)?;
                let target = if up { index.checked_sub(1)? } else { index + 1 };
                if target >= list.len() {
                    return None;
                }
                list.swap(index, target);
                let mut moved = path.clone();
                *moved.last_mut()? = Step::Stage(target);
                if let Some(at) = self.patch_lines().iter().position(|line| *line == PatchLine::Stage(moved.clone())) {
                    self.patch_cursor = at;
                }
                return self.changed();
            }
            _ => {}
        }
        None
    }

    fn patch_delete(&mut self) -> Option<EffectsSettings> {
        match self.patch_current()? {
            PatchLine::Modulator(index) | PatchLine::ModulatorParam(index, _) => {
                let patch = self.patch();
                patch.modulators.remove(index);
                // Settings it moved are fixed again; later sources move up.
                fn fix(stages: &mut [Stage], gone: usize) {
                    for stage in stages {
                        match stage {
                            Stage::Block(block) => {
                                block.modulations.retain(|m| m.source != gone);
                                for m in &mut block.modulations {
                                    if m.source > gone {
                                        m.source -= 1;
                                    }
                                }
                            }
                            Stage::Parallel(branches) => branches.iter_mut().for_each(|b| fix(&mut b.stages, gone)),
                            Stage::Feedback { stages, .. } => fix(stages, gone),
                        }
                    }
                }
                fix(&mut patch.stages, index);
            }
            PatchLine::Stage(path) | PatchLine::Param(path, _) => {
                let (list, index) = container(&mut self.patch().stages, &path)?;
                list.remove(index);
            }
            PatchLine::Branch(path) => {
                let Some(Step::Branch(branch)) = path.last().copied() else { return None };
                if let Some(Stage::Parallel(branches)) = stage_at(&mut self.patch().stages, &path[..path.len() - 1]) {
                    branches.remove(branch);
                }
            }
        }
        self.patch_cursor = self.patch_cursor.min(self.patch_lines().len().saturating_sub(1));
        self.changed()
    }

    fn patch_adjust(&mut self, direction: f32) -> Option<EffectsSettings> {
        match self.patch_current()? {
            PatchLine::ModulatorParam(index, param) => {
                let modulator = &mut self.patch().modulators[index];
                let spec = modulator.kind.params()[param];
                let value = step_value(&spec, modulator.params.get(spec.id).copied().unwrap_or(spec.default), direction);
                modulator.params.insert(spec.id.to_owned(), value);
            }
            PatchLine::Param(path, param) => {
                let Some(Stage::Block(block)) = stage_at(&mut self.patch().stages, &path) else { return None };
                let spec = block.kind.params()[param];
                let value = step_value(&spec, block.params.get(spec.id).copied().unwrap_or(spec.default), direction);
                block.params.insert(spec.id.to_owned(), value);
            }
            PatchLine::Branch(path) => {
                let Some(Step::Branch(branch)) = path.last().copied() else { return None };
                let Some(Stage::Parallel(branches)) = stage_at(&mut self.patch().stages, &path[..path.len() - 1]) else { return None };
                let level = &mut branches.get_mut(branch)?.level;
                *level = (*level + 0.05 * direction).clamp(0.0, 2.0);
            }
            PatchLine::Stage(path) => match stage_at(&mut self.patch().stages, &path)? {
                Stage::Feedback { amount, .. } => *amount = (*amount + 0.02 * direction).clamp(0.0, 0.98),
                Stage::Block(block) => block.enabled = !block.enabled,
                Stage::Parallel(_) => return None,
            },
            PatchLine::Modulator(_) => return None,
        }
        self.changed()
    }

    /// Which modulator moves the selected setting: none, then each in turn.
    fn cycle_modulation(&mut self) -> Option<EffectsSettings> {
        let PatchLine::Param(path, param) = self.patch_current()? else { return None };
        let sources = self.patch().modulators.len();
        let Some(Stage::Block(block)) = stage_at(&mut self.patch().stages, &path) else { return None };
        let spec = block.kind.params()[param];
        if !spec.choices.is_empty() || sources == 0 {
            return None;
        }
        let current = block.modulations.iter().position(|m| m.param == spec.id);
        let next = match current {
            None => Some(0),
            Some(at) if block.modulations[at].source + 1 < sources => Some(block.modulations[at].source + 1),
            Some(_) => None,
        };
        block.modulations.retain(|m| m.param != spec.id);
        if let Some(source) = next {
            block.modulations.push(Modulation { param: spec.id.to_owned(), source, depth: 0.2 });
        }
        self.changed()
    }

    fn modulation_depth(&mut self, change: f32) -> Option<EffectsSettings> {
        let PatchLine::Param(path, param) = self.patch_current()? else { return None };
        let Some(Stage::Block(block)) = stage_at(&mut self.patch().stages, &path) else { return None };
        let id = block.kind.params()[param].id;
        let modulation = block.modulations.iter_mut().find(|m| m.param == id)?;
        modulation.depth = ((modulation.depth + change) * 100.0).round() / 100.0;
        modulation.depth = modulation.depth.clamp(-1.0, 1.0);
        self.changed()
    }

    // -----------------------------------------------------------------------
    // Drawing

    pub fn draw(&self, out: &mut String, size: (usize, usize)) {
        if self.editing.is_some() {
            return self.draw_patch(out, size);
        }
        let (width, height) = size;
        out.push_str("\x1b[2J");
        paint(out, 1, 1, " Effects · after the equalizer, top to bottom", width, Surface::Toolbar, true);
        let mut rows: Vec<(String, Option<Line>)> = vec![
            (format!("Use effects: {}", if self.settings.enabled { "On" } else { "Off" }), Some(Line::Enabled)),
            (
                format!("Preset: {}", if self.settings.preset_name.is_empty() { "Custom" } else { &self.settings.preset_name }),
                Some(Line::Preset),
            ),
        ];
        for (slot, effect) in self.settings.chain.iter().enumerate() {
            let label = match (&effect.kind, &effect.patch) {
                (EffectKind::Custom, Some(name)) => {
                    let missing = !self.settings.patches.iter().any(|patch| &patch.name == name);
                    format!("{name}{}", if missing { " (missing)" } else { "" })
                }
                _ => effect.kind.label().to_owned(),
            };
            rows.push((format!("{}. {label} [{}]", slot + 1, if effect.enabled { "on" } else { "off" }), Some(Line::Slot(slot))));
            for (param, spec) in effect.kind.params().iter().enumerate() {
                let value = effect.get(spec.id);
                rows.push((format!("     {:<16} {} {}", spec.label, bar(spec, value), shown(spec, value)), Some(Line::Param(slot, param))));
            }
        }
        if self.settings.chain.is_empty() {
            rows.push(("No effects. Press A to add one, or pick a preset.".to_owned(), None));
        }
        rows.push((String::new(), None));
        rows.push(("Custom effects (E or Enter to edit, N for a new one):".to_owned(), None));
        for (index, patch) in self.settings.patches.iter().enumerate() {
            rows.push((format!("  {} · {} blocks", patch.name, patch.block_count()), Some(Line::Patch(index))));
        }
        let current = self.current();
        let rows: Vec<_> = rows
            .iter()
            .map(|(text, line)| (text.clone(), *line == Some(current), line.is_none() || matches!(line, Some(Line::Slot(_) | Line::Patch(_)))))
            .collect();
        self.draw_rows(out, size, &rows);
        let help = match self.picking {
            Some(Picking::Effect) => EFFECT_KINDS.iter().enumerate().map(|(i, kind)| format!("{} {}", &PICK_KEYS[i..=i], kind.label())).collect::<Vec<_>>().join(" · "),
            Some(Picking::Custom) => self.settings.patches.iter().enumerate().take(PICK_KEYS.len()).map(|(i, patch)| format!("{} {}", &PICK_KEYS[i..=i], patch.name)).collect::<Vec<_>>().join(" · "),
            Some(Picking::Template) => std::iter::once("Empty".to_owned())
                .chain(templates().into_iter().map(|patch| patch.name))
                .enumerate()
                .take(PICK_KEYS.len())
                .map(|(i, name)| format!("{} {name}", &PICK_KEYS[i..=i]))
                .collect::<Vec<_>>()
                .join(" · "),
            _ => "↑↓ select · ←→ adjust · Space on/off · A add · C add custom · N new custom · E edit · D remove · [ ] move · Esc close".to_owned(),
        };
        paint(out, height, 2, &help, width.saturating_sub(3), Surface::Muted, false);
    }

    fn draw_patch(&self, out: &mut String, size: (usize, usize)) {
        let (width, height) = size;
        out.push_str("\x1b[2J");
        let Some(patch) = self.editing.and_then(|index| self.settings.patches.get(index)) else { return };
        paint(out, 1, 1, &format!(" Custom effect · {}", patch.name), width, Surface::Toolbar, true);
        let lines = self.patch_lines();
        let current = self.patch_current();
        let mut rows = Vec::new();
        let mut patch_clone = patch.clone();
        for line in &lines {
            let depth = |path: &[Step]| path.iter().filter(|step| matches!(step, Step::Stage(_))).count().saturating_sub(1) * 2
                + path.iter().filter(|step| matches!(step, Step::Branch(_))).count() * 2;
            let text = match line {
                PatchLine::Modulator(index) => format!("{} {}", patch.modulators[*index].kind.label(), index + 1),
                PatchLine::ModulatorParam(index, param) => {
                    let modulator = &patch.modulators[*index];
                    let spec = modulator.kind.params()[*param];
                    let value = modulator.params.get(spec.id).copied().unwrap_or(spec.default);
                    format!("     {:<16} {} {}", spec.label, bar(&spec, value), shown(&spec, value))
                }
                PatchLine::Stage(path) => {
                    let indent = " ".repeat(depth(path) * 2);
                    match stage_at(&mut patch_clone.stages, path) {
                        Some(Stage::Block(block)) => format!("{indent}■ {} [{}]", block.kind.label(), if block.enabled { "on" } else { "off" }),
                        Some(Stage::Parallel(branches)) => format!("{indent}╦ Parallel paths ({})", branches.len()),
                        Some(Stage::Feedback { amount, .. }) => format!("{indent}↻ Feedback loop · amount {amount:.2}"),
                        None => String::new(),
                    }
                }
                PatchLine::Branch(path) => {
                    let indent = " ".repeat(depth(path) * 2);
                    let Some(Step::Branch(branch)) = path.last().copied() else { continue };
                    let level = match stage_at(&mut patch_clone.stages, &path[..path.len() - 1]) {
                        Some(Stage::Parallel(branches)) => branches.get(branch).map_or(0.0, |b| b.level),
                        _ => 0.0,
                    };
                    format!("{indent}├ Path {} · level {level:.2}", branch + 1)
                }
                PatchLine::Param(path, param) => {
                    let indent = " ".repeat(depth(path) * 2 + 5);
                    match stage_at(&mut patch_clone.stages, path) {
                        Some(Stage::Block(block)) => {
                            let spec = block.kind.params()[*param];
                            let value = block.params.get(spec.id).copied().unwrap_or(spec.default);
                            let moved = block
                                .modulations
                                .iter()
                                .find(|m| m.param == spec.id)
                                .map(|m| format!("  ~ {} {} at {:+.0}%", patch.modulators[m.source].kind.label(), m.source + 1, m.depth * 100.0))
                                .unwrap_or_default();
                            format!("{indent}{:<16} {} {}{moved}", spec.label, bar(&spec, value), shown(&spec, value))
                        }
                        _ => String::new(),
                    }
                }
            };
            let heading = matches!(line, PatchLine::Modulator(_) | PatchLine::Stage(_) | PatchLine::Branch(_));
            rows.push((text, Some(line) == current.as_ref(), heading));
        }
        if rows.is_empty() {
            rows.push(("Empty. A adds a block, P parallel paths, F a feedback loop, L an LFO, V an envelope follower.".to_owned(), true, false));
        }
        self.draw_rows(out, size, &rows);
        let help = if self.picking == Some(Picking::Block) {
            BLOCK_KINDS.iter().enumerate().map(|(i, kind)| format!("{} {}", &PICK_KEYS[i..=i], kind.label())).collect::<Vec<_>>().join(" · ")
        } else {
            "↑↓ select · ←→ adjust · A block · P parallel · B path · F feedback · L LFO · V envelope · M move-by · , . depth · D remove · [ ] move · Esc back".to_owned()
        };
        paint(out, height, 2, &help, width.saturating_sub(3), Surface::Muted, false);
    }

    fn draw_rows(&self, out: &mut String, size: (usize, usize), rows: &[(String, bool, bool)]) {
        let (width, height) = size;
        let visible = height.saturating_sub(4).max(1);
        let selected = rows.iter().position(|(_, current, _)| *current).unwrap_or(0);
        let first = selected.saturating_sub(visible.saturating_sub(1));
        for (offset, (text, current, heading)) in rows.iter().enumerate().skip(first).take(visible) {
            let surface = if *current { Surface::Selected } else { Surface::Main };
            paint(out, 3 + offset - first, 2, text, width.saturating_sub(3), surface, *heading);
        }
    }
}
