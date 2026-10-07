use super::{Key, Surface, paint, truncate};
use kog_audio::inspection::{Snapshot, note_name};

pub(super) struct View {
    pub open: bool,
    mode: u8,
    channel: usize,
    row: usize,
    follow: bool,
    octave: usize,
    detail: bool,
    scroll: usize,
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
        }
    }
}
impl View {
    pub fn key(&mut self, key: Key) {
        match key {
            Key::Esc | Key::Char('q') => self.open = false,
            Key::Char('1') => self.mode = 1,
            Key::Char('2') => self.mode = 2,
            Key::Char('3') => self.mode = 3,
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
    pub fn wheel(&mut self, down: bool) {
        self.key(if down { Key::Down } else { Key::Up });
    }
    pub fn draw(&mut self, out: &mut String, size: (usize, usize), state: &Snapshot) {
        let (width, height) = size;
        if width < 24 || height < 10 {
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
                "[1 Keyboards] [2 Tracker] [3 Both] [D Details] [F Follow: {}] [Esc Close]",
                if self.follow { "on" } else { "off" }
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
        let available = height.saturating_sub(5);
        let keyboard_height = if self.mode == 1 {
            available
        } else if self.mode == 3 {
            available / 2
        } else {
            0
        };
        if keyboard_height > 0 {
            for (n, channel) in state
                .channels
                .iter()
                .skip(self.channel)
                .take(keyboard_height / 3)
                .enumerate()
            {
                let y = 4 + n * 3;
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
                paint(
                    out,
                    y,
                    2,
                    &text,
                    width - 3,
                    if n == 0 {
                        Surface::Selected
                    } else {
                        Surface::Main
                    },
                    n == 0,
                );
                let octaves = ((width - 4) / 14).clamp(1, 10);
                let first = (self.octave * 12).min(120);
                let white = [0, 2, 4, 5, 7, 9, 11];
                for octave in 0..octaves {
                    for (index, offset) in white.iter().enumerate() {
                        let key = first + octave * 12 + offset;
                        if key > 127 {
                            continue;
                        }
                        let x = 2 + octave * 14 + index * 2;
                        let active = channel.notes.iter().find(|n| n.key.round() as usize == key);
                        let bg = match active {
                            Some(n) if n.held => "79;195;247",
                            Some(_) => "112;217;170",
                            None => "224;227;233",
                        };
                        out.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;2;25;29;37;48;2;{}m  \x1b[{};{}H{}\x1b[0m",
                            y + 1,
                            x,
                            bg,
                            y + 2,
                            x,
                            if *offset == 0 {
                                format!("C{}", key as i32 / 12 - 1)
                            } else {
                                "  ".into()
                            }
                        ));
                    }
                }
                for octave in 0..octaves {
                    for (offset, xoff) in [(1, 1), (3, 3), (6, 7), (8, 9), (10, 11)] {
                        let key = first + octave * 12 + offset;
                        if key > 127 {
                            continue;
                        }
                        let active = channel.notes.iter().find(|n| n.key.round() as usize == key);
                        let bg = match active {
                            Some(n) if n.held => "79;195;247",
                            Some(_) => "112;217;170",
                            None => "15;19;24",
                        };
                        out.push_str(&format!(
                            "\x1b[{};{}H\x1b[48;2;{}m \x1b[0m",
                            y + 1,
                            2 + octave * 14 + xoff,
                            bg
                        ));
                    }
                }
            }
        }
        if self.mode != 1 {
            let y = 4 + keyboard_height;
            let count = (width.saturating_sub(15) / 23).max(1);
            let channels = state
                .channels
                .iter()
                .skip(self.channel)
                .take(count)
                .collect::<Vec<_>>();
            paint(out, y, 2, "Time / row", 12, Surface::Header, true);
            for (n, c) in channels.iter().enumerate() {
                paint(out, y, 15 + n * 23, &c.name, 22, Surface::Header, true);
            }
            let rows = height.saturating_sub(y + 2);
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
                    paint(out, line, 15 + index * 23, &cell, 22, surface, false);
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
            .min(lines.len().saturating_sub(size.1.saturating_sub(5)));
        for (n, line) in lines.iter().skip(self.scroll).take(size.1 - 5).enumerate() {
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
