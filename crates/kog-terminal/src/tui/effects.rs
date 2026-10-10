//! The effects chain in the terminal: a full-screen list of effects and their
//! settings, edited with the keyboard and applied as it changes.
use super::{Key, Surface, paint};
use kog_core::effects::{EFFECT_KINDS, EffectSlot, EffectsSettings, MAX_EFFECTS, presets};

/// One selectable line of the screen.
#[derive(Clone, Copy, PartialEq)]
enum Line {
    Enabled,
    Preset,
    Slot(usize),
    Param(usize, usize),
}

#[derive(Default)]
pub(super) struct Effects {
    pub open: bool,
    settings: EffectsSettings,
    cursor: usize,
    /// Picking an effect to add: the digit keys choose one.
    adding: bool,
}

impl Effects {
    pub fn show(&mut self, settings: EffectsSettings) {
        self.settings = settings;
        self.open = true;
        self.adding = false;
        self.cursor = self.cursor.min(self.lines().len().saturating_sub(1));
    }

    fn lines(&self) -> Vec<Line> {
        let mut lines = vec![Line::Enabled, Line::Preset];
        for (slot, effect) in self.settings.chain.iter().enumerate() {
            lines.push(Line::Slot(slot));
            lines.extend((0..effect.kind.params().len()).map(|param| Line::Param(slot, param)));
        }
        lines
    }

    fn current(&self) -> Line {
        let lines = self.lines();
        lines[self.cursor.min(lines.len() - 1)]
    }

    fn select_slot(&mut self, slot: usize) {
        if let Some(at) = self.lines().iter().position(|line| *line == Line::Slot(slot)) {
            self.cursor = at;
        }
    }

    /// Handle a key; returns the new settings when they changed.
    pub fn key(&mut self, key: Key) -> Option<EffectsSettings> {
        if self.adding {
            self.adding = false;
            if let Key::Char(digit @ '1'..='8') = key {
                let kind = EFFECT_KINDS[digit as usize - '1' as usize];
                self.settings.chain.push(EffectSlot::new(kind));
                self.settings.enabled = true;
                self.settings.preset_name.clear();
                self.select_slot(self.settings.chain.len() - 1);
                return Some(self.settings.clone());
            }
            return None;
        }
        let lines = self.lines().len();
        match key {
            Key::Esc | Key::Char('q') => self.open = false,
            Key::Up => self.cursor = self.cursor.saturating_sub(1),
            Key::Down => self.cursor = (self.cursor + 1).min(lines - 1),
            Key::Char('a') if self.settings.chain.len() < MAX_EFFECTS => self.adding = true,
            Key::Left | Key::Char('-') => return self.adjust(-1.0),
            Key::Right | Key::Char('+') | Key::Char('=') => return self.adjust(1.0),
            Key::Char(' ') | Key::Enter => return self.toggle(),
            Key::Char('d') | Key::Delete => {
                let (Line::Slot(slot) | Line::Param(slot, _)) = self.current() else { return None };
                self.settings.chain.remove(slot);
                self.settings.preset_name.clear();
                self.cursor = self.cursor.min(self.lines().len() - 1);
                return Some(self.settings.clone());
            }
            Key::Char('[') | Key::Char(']') => {
                let (Line::Slot(slot) | Line::Param(slot, _)) = self.current() else { return None };
                let target = if key == Key::Char('[') { slot.checked_sub(1)? } else { slot + 1 };
                if target >= self.settings.chain.len() {
                    return None;
                }
                self.settings.chain.swap(slot, target);
                self.settings.preset_name.clear();
                self.select_slot(target);
                return Some(self.settings.clone());
            }
            _ => {}
        }
        None
    }

    fn toggle(&mut self) -> Option<EffectsSettings> {
        match self.current() {
            Line::Enabled => self.settings.enabled = !self.settings.enabled,
            Line::Slot(slot) => self.settings.chain[slot].enabled = !self.settings.chain[slot].enabled,
            Line::Preset => return self.adjust(1.0),
            Line::Param(..) => return None,
        }
        Some(self.settings.clone())
    }

    fn adjust(&mut self, direction: f32) -> Option<EffectsSettings> {
        match self.current() {
            Line::Enabled | Line::Slot(_) => return self.toggle(),
            Line::Preset => {
                let presets = presets();
                let at = presets.iter().position(|preset| preset.preset_name == self.settings.preset_name);
                let next = match (at, direction > 0.0) {
                    (Some(at), true) => (at + 1) % presets.len(),
                    (Some(at), false) => (at + presets.len() - 1) % presets.len(),
                    (None, _) => 0,
                };
                self.settings = presets[next].clone();
            }
            Line::Param(slot, param) => {
                let effect = &mut self.settings.chain[slot];
                let spec = effect.kind.params()[param];
                // Fifty steps across the range, at least one unit step.
                let step = ((spec.max - spec.min) / 50.0).max(spec.step);
                let value = (effect.get(spec.id) + step * direction).clamp(spec.min, spec.max);
                let value = (value / spec.step).round() * spec.step;
                effect.params.insert(spec.id.to_owned(), value);
                self.settings.preset_name.clear();
            }
        }
        Some(self.settings.clone())
    }

    pub fn draw(&self, out: &mut String, size: (usize, usize)) {
        let (width, height) = size;
        out.push_str("\x1b[2J");
        paint(out, 1, 1, " Effects · after the equalizer, top to bottom", width, Surface::Toolbar, true);
        let mut rows: Vec<(String, Line)> = vec![
            (format!("Use effects: {}", if self.settings.enabled { "On" } else { "Off" }), Line::Enabled),
            (
                format!(
                    "Preset: {}",
                    if self.settings.preset_name.is_empty() { "Custom" } else { &self.settings.preset_name }
                ),
                Line::Preset,
            ),
        ];
        for (slot, effect) in self.settings.chain.iter().enumerate() {
            rows.push((
                format!("{}. {} [{}]", slot + 1, effect.kind.label(), if effect.enabled { "on" } else { "off" }),
                Line::Slot(slot),
            ));
            for (param, spec) in effect.kind.params().iter().enumerate() {
                let value = effect.get(spec.id);
                let filled = (((value - spec.min) / (spec.max - spec.min)) * 20.0).round() as usize;
                let shown = if spec.id == "ping_pong" {
                    if value >= 0.5 { "On".to_owned() } else { "Off".to_owned() }
                } else {
                    let digits = if spec.step >= 1.0 { 0 } else if spec.step >= 0.1 { 1 } else { 2 };
                    format!("{value:.digits$} {}", spec.unit)
                };
                rows.push((
                    format!("     {:<16} {}{} {shown}", spec.label, "█".repeat(filled), "·".repeat(20 - filled.min(20))),
                    Line::Param(slot, param),
                ));
            }
        }
        if self.settings.chain.is_empty() {
            rows.push(("No effects. Press A to add one, or pick a preset.".to_owned(), Line::Preset));
        }
        let current = self.current();
        let visible = height.saturating_sub(4).max(1);
        let selected = rows.iter().position(|(_, line)| *line == current).unwrap_or(0);
        let first = selected.saturating_sub(visible.saturating_sub(1));
        for (offset, (text, line)) in rows.iter().enumerate().skip(first).take(visible) {
            let surface = if *line == current { Surface::Selected } else { Surface::Main };
            paint(out, 3 + offset - first, 2, text, width.saturating_sub(3), surface, matches!(line, Line::Slot(_)));
        }
        let help = if self.adding {
            EFFECT_KINDS
                .iter()
                .enumerate()
                .map(|(index, kind)| format!("{} {}", index + 1, kind.label()))
                .collect::<Vec<_>>()
                .join(" · ")
        } else {
            "↑↓ select · ←→ adjust · Space on/off · A add · D remove · [ ] move · Esc close".to_owned()
        };
        paint(out, height, 2, &help, width.saturating_sub(3), Surface::Muted, false);
    }
}
