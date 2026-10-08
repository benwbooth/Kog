use super::{Key, Surface, paint, truncate};
use kog_audio::inspection::{Channel, Snapshot, note_name};

pub(super) struct View {
    pub open: bool,
    mode: u8,
    channel: usize,
    row: usize,
    follow: bool,
    octave: usize,
    detail: bool,
    scroll: usize,
    pub mml: super::mml::Mml,
    guide: super::guide::Guide,
}
impl Default for View {
    fn default() -> Self {
        Self {
            open: false,
            mode: 3,
            channel: 0,
            row: 0,
            follow: true,
            octave: 2,
            detail: false,
            scroll: 0,
            mml: super::mml::Mml::default(),
            guide: super::guide::Guide::default(),
        }
    }
}
impl View {
    /// Open on one view: 1 keyboards, 2 tracker, 3 both, 4 MML.
    pub fn show(&mut self, mode: u8) {
        self.mode = mode;
        self.open = true;
    }

    pub fn key(&mut self, key: Key) {
        if self.guide.open {
            self.guide.key(key);
            return;
        }
        match key {
            Key::Char('g') | Key::Char('G') if self.mode == 4 => self.guide.open = true,
            Key::Esc | Key::Char('q') => self.open = false,
            Key::Char('1') => self.mode = 1,
            Key::Char('2') => self.mode = 2,
            Key::Char('3') => self.mode = 3,
            Key::Char('4') => self.mode = 4,
            Key::Char('f') if self.mode == 4 => self.mml.follow = !self.mml.follow,
            Key::Char('+') | Key::Char('=') if self.mode == 4 => self.mml.change_bars(true),
            Key::Char('-') if self.mode == 4 => self.mml.change_bars(false),
            Key::Char('y') | Key::Char('Y') if self.mode == 4 => self.mml.copy(),
            Key::Char('e') | Key::Char('E') if self.mode == 4 => self.mml.export(),
            Key::Up if self.mode == 4 => {
                self.mml.follow = false;
                self.mml.scroll = self.mml.scroll.saturating_sub(1);
            }
            Key::Down if self.mode == 4 => {
                self.mml.follow = false;
                self.mml.scroll = self.mml.scroll.saturating_add(1);
            }
            Key::PageUp if self.mode == 4 => {
                self.mml.follow = false;
                self.mml.scroll = self.mml.scroll.saturating_sub(16);
            }
            Key::PageDown if self.mode == 4 => {
                self.mml.follow = false;
                self.mml.scroll = self.mml.scroll.saturating_add(16);
            }
            Key::Enter | Key::Char('d') => {
                self.detail = !self.detail;
                self.scroll = 0;
            }
            Key::Char('f') => self.follow = !self.follow,
            Key::Char('[') => self.octave = self.octave.saturating_sub(1),
            Key::Char(']') => self.octave = (self.octave + 1).min(9),
            Key::Left => self.channel = self.channel.saturating_sub(1),
            Key::Right => self.channel = self.channel.saturating_add(1),
            Key::Up if self.detail => self.scroll = self.scroll.saturating_sub(1),
            Key::Down if self.detail => self.scroll = self.scroll.saturating_add(1),
            Key::Up if self.mode == 2 => {
                self.follow = false;
                self.row = self.row.saturating_sub(1);
            }
            Key::Down if self.mode == 2 => {
                self.follow = false;
                self.row = self.row.saturating_add(1);
            }
            Key::Up => self.channel = self.channel.saturating_sub(1),
            Key::Down => self.channel = self.channel.saturating_add(1),
            Key::PageUp => {
                self.follow = false;
                self.row = self.row.saturating_sub(8);
            }
            Key::PageDown => {
                self.follow = false;
                self.row = self.row.saturating_add(8);
            }
            _ => {}
        }
    }
    /// The MML view records the whole song, so it is only started on demand.
    pub fn wants_mml(&self) -> bool {
        self.open && self.mode == 4
    }
    pub fn wheel(&mut self, down: bool) {
        self.key(if down { Key::Down } else { Key::Up });
    }
    pub fn draw(&mut self, out: &mut String, size: (usize, usize), state: &Snapshot) {
        let (width, height) = size;
        if width < 24 || height < 10 {
            return;
        }
        if self.guide.open {
            self.guide.draw(out, size);
            return;
        }
        for y in 1..=height {
            paint(out, y, 1, &" ".repeat(width), width, Surface::Main, false);
        }
        paint(
            out,
            1,
            2,
            &format!(
                "Channel Inspector · {} · {:.3}s {}",
                state.description.backend,
                state.position,
                if state.playing { "▶" } else { "❚❚" }
            ),
            width - 3,
            Surface::Accent,
            true,
        );
        paint(
            out,
            2,
            2,
            &format!(
                "[1 Keyboards] [2 Tracker] [3 Both] [4 MML] [D Details] [F Follow: {}] [Esc Close]",
                if (self.mode == 4 && self.mml.follow) || (self.mode != 4 && self.follow) { "on" } else { "off" }
            ),
            width - 3,
            Surface::Header,
            false,
        );
        paint(
            out,
            3,
            2,
            &state.description.detail,
            width - 3,
            Surface::Muted,
            false,
        );
        if self.mode == 4 {
            self.mml
                .draw(out, (2, 4, width - 3, height.saturating_sub(4)), state.position);
            paint(
                out,
                height,
                2,
                &if self.mml.notice.is_empty() {
                    format!(
                        "Highlighted notes are sounding · G guide · +/- bars per line ({}) · Y copy · E export · ↑↓ scroll · F follow · Space play/pause",
                        self.mml.bars_label()
                    )
                } else {
                    format!("{} · G guide · Y copy · E export · F follow", self.mml.notice)
                },
                width - 3,
                Surface::Muted,
                false,
            );
            return;
        }
        self.channel = self.channel.min(state.channels.len().saturating_sub(1));
        self.row = self.row.min(state.rows.len().saturating_sub(1));
        if self.follow {
            self.row = state.current_row.unwrap_or(0);
        }
        if state.channels.is_empty() {
            paint(
                out,
                6,
                3,
                if state.seeking {
                    "Seeking…"
                } else {
                    "Waiting for musical channel data…"
                },
                width - 4,
                Surface::Muted,
                false,
            );
            return;
        }
        if self.detail {
            self.draw_details(out, size, state);
            return;
        }
        let available = height.saturating_sub(4);
        let keyboard_height = if self.mode == 1 {
            available
        } else if self.mode == 3 {
            available / 2
        } else {
            0
        };
        if keyboard_height > 0 {
            let visible = (keyboard_height / 3).min(state.channels.len() - self.channel);
            for (n, channel) in state
                .channels
                .iter()
                .skip(self.channel)
                .take(visible)
                .enumerate()
            {
                let top = n * keyboard_height / visible;
                let bottom = (n + 1) * keyboard_height / visible;
                let y = 4 + top;
                let notes = channel
                    .notes
                    .iter()
                    .map(|n| format!("{}{}", note_name(n.key), if n.held { "" } else { "~" }))
                    .collect::<Vec<_>>()
                    .join(" ");
                let text = format!(
                    "{} · {} · {}",
                    channel.name,
                    channel.instrument,
                    if notes.is_empty() {
                        if channel.active {
                            channel.kind.to_uppercase()
                        } else {
                            "—".into()
                        }
                    } else {
                        notes
                    }
                );
                let meter = level_meter(channel.level, if width >= 50 { 10 } else { 4 });
                let meter_width = meter.chars().count();
                paint(
                    out,
                    y,
                    2,
                    &text,
                    width - meter_width - 4,
                    if n == 0 {
                        Surface::Selected
                    } else {
                        Surface::Main
                    },
                    n == 0,
                );
                paint(
                    out,
                    y,
                    width - meter_width - 1,
                    &meter,
                    meter_width,
                    Surface::Accent,
                    false,
                );
                draw_keyboard(
                    out,
                    channel,
                    (2, y + 1, width - 3, bottom - top - 1),
                    (self.octave * 12).min(120),
                );
            }
        }
        if self.mode != 1 {
            let y = 4 + keyboard_height;
            let column_space = width - 16;
            let count = (column_space / 23)
                .max(1)
                .min(state.channels.len() - self.channel);
            let column = |index: usize| {
                let start = index * column_space / count;
                let end = (index + 1) * column_space / count;
                (15 + start, end - start)
            };
            let channels = state
                .channels
                .iter()
                .skip(self.channel)
                .take(count)
                .collect::<Vec<_>>();
            paint(out, y, 2, "Time / row", 12, Surface::Header, true);
            for (n, c) in channels.iter().enumerate() {
                let (x, width) = column(n);
                paint(out, y, x, &c.name, width - 1, Surface::Header, true);
            }
            let rows = height.saturating_sub(y + 1);
            let first = self.row.saturating_sub(rows / 2);
            for (n, row) in state.rows.iter().enumerate().skip(first).take(rows) {
                let line = y + 1 + n - first;
                let surface = if state.current_row == Some(n) {
                    Surface::Selected
                } else {
                    Surface::Main
                };
                paint(out, line, 2, &row.label, 12, surface, false);
                for (index, c) in channels.iter().enumerate() {
                    let cell = row
                        .cells
                        .iter()
                        .filter(|cell| cell.channel == c.id)
                        .map(|cell| {
                            let effects = cell
                                .effects
                                .iter()
                                .map(|f| format!("{} {}", f.name, f.value))
                                .collect::<Vec<_>>()
                                .join(" ");
                            format!(
                                "{} {} {} {}",
                                cell.notes, cell.instrument, cell.volume, effects
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" · ");
                    let (x, width) = column(index);
                    paint(out, line, x, &cell, width - 1, surface, false);
                }
            }
        }
        paint(
            out,
            height,
            2,
            "↑↓ scroll · ←→ channels · [ ] octaves · PgUp/PgDn rows · D full effects · Space play/pause",
            width - 3,
            Surface::Muted,
            false,
        );
    }
    fn draw_details(&mut self, out: &mut String, size: (usize, usize), state: &Snapshot) {
        let channel = &state.channels[self.channel];
        let mut lines = vec![
            format!("{} · {}", channel.name, channel.instrument),
            level_meter(channel.level, 10),
            format!(
                "Notes: {}",
                channel
                    .notes
                    .iter()
                    .map(|n| format!(
                        "{} ({:.2}){}",
                        note_name(n.key),
                        n.key,
                        if n.held { "" } else { " sustained" }
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ];
        lines.extend(
            channel
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, f.value)),
        );
        if let Some(row) = state.rows.get(self.row) {
            lines.push(format!("Row {} · {:.3}s", row.label, row.time));
            for cell in row.cells.iter().filter(|cell| cell.channel == channel.id) {
                lines.push(format!(
                    "{} · {} · Volume {}",
                    cell.notes, cell.instrument, cell.volume
                ));
                lines.extend(
                    cell.effects
                        .iter()
                        .map(|f| format!("{}: {}", f.name, f.value)),
                );
            }
            lines.extend(
                row.global
                    .iter()
                    .map(|f| format!("{}: {}", f.name, f.value)),
            );
        }
        lines.extend(
            state
                .global
                .iter()
                .map(|f| format!("{}: {}", f.name, f.value)),
        );
        self.scroll = self
            .scroll
            .min(lines.len().saturating_sub(size.1.saturating_sub(4)));
        for (n, line) in lines.iter().skip(self.scroll).take(size.1 - 4).enumerate() {
            paint(
                out,
                4 + n,
                2,
                &truncate(line, size.0 - 3),
                size.0 - 3,
                Surface::Main,
                false,
            );
        }
        paint(
            out,
            size.1,
            2,
            "↑↓ scroll · ←→ channel · D return · Esc close · Space play/pause",
            size.0 - 3,
            Surface::Muted,
            false,
        );
    }
}

/// Divide the entire pane into white keys; wider or taller terminals grow
/// the keys after the visible pitch range has reached MIDI's upper limit.
fn draw_keyboard(
    out: &mut String,
    channel: &Channel,
    area: (usize, usize, usize, usize),
    first: usize,
) {
    let (x, y, width, height) = area;
    let black = |key: usize| matches!(key % 12, 1 | 3 | 6 | 8 | 10);
    let whites = (first..128)
        .filter(|key| !black(*key))
        .take(width / 2)
        .collect::<Vec<_>>();
    let color = |key: usize, idle| match channel
        .notes
        .iter()
        .find(|note| note.key.round() as usize == key)
    {
        Some(note) if note.held => "79;195;247",
        Some(_) => "112;217;170",
        None => idle,
    };
    for (index, key) in whites.iter().enumerate() {
        let start = index * width / whites.len();
        let end = (index + 1) * width / whites.len();
        let key_width = end - start;
        let fill = if index == 0 {
            " ".repeat(key_width)
        } else {
            format!("│{}", " ".repeat(key_width - 1))
        };
        let bg = color(*key, "224;227;233");
        for row in 0..height {
            out.push_str(&format!(
                "\x1b[{};{}H\x1b[38;2;70;77;85;48;2;{}m{}\x1b[0m",
                y + row,
                x + start,
                bg,
                fill
            ));
        }
        if key % 12 == 0 {
            out.push_str(&format!(
                "\x1b[{};{}H\x1b[38;2;25;29;37;48;2;{}m{}\x1b[0m",
                y + height - 1,
                x + start,
                bg,
                truncate(&format!("C{}", *key as i32 / 12 - 1), key_width)
            ));
        }
    }
    for (index, key) in whites
        .iter()
        .enumerate()
        .take(whites.len().saturating_sub(1))
    {
        if !black(key + 1) {
            continue;
        }
        let start = index * width / whites.len();
        let end = (index + 1) * width / whites.len();
        let black_width = (end - start).div_ceil(2);
        let black_height = (height * 3 / 5).max(1).min(height - 1);
        let fill = " ".repeat(black_width);
        let bg = color(key + 1, "15;19;24");
        for row in 0..black_height {
            out.push_str(&format!(
                "\x1b[{};{}H\x1b[48;2;{}m{}\x1b[0m",
                y + row,
                x + end - black_width.div_ceil(2),
                bg,
                fill
            ));
        }
    }
}

fn level_meter(level: f32, cells: usize) -> String {
    let level = if level.is_finite() {
        level.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = (level * cells as f32).round() as usize;
    format!(
        "Level {}{} {:3.0}%",
        "█".repeat(filled),
        "░".repeat(cells - filled),
        level * 100.0
    )
}
