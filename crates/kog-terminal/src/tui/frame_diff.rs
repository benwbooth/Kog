//! Draw code builds each frame as a string of absolute-positioned paints, and
//! one frame often paints the same cell more than once (a row background, then
//! its text, then a highlight). Writing that string straight to the terminal
//! made it repaint the whole screen on every arrow press, and terminals showed
//! the half-painted states as flicker.
//!
//! `FrameDiff` keeps a model of what the terminal shows, plays each new frame
//! onto a copy of it, and writes only the cells that ended up different, inside
//! synchronized output so terminals that support it show the frame at once.
//!
//! It understands the sequences the draw code uses: cursor position (`H`),
//! SGR (`m`), clear screen (`2J`), erase line (`K`) and cursor visibility
//! (`?25h` / `?25l`). Anything else is dropped.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum Color {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
struct Style {
    fg: Color,
    bg: Color,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    reverse: bool,
}

impl Style {
    fn apply_sgr(&mut self, params: &str) {
        let values: Vec<u16> = if params.is_empty() {
            vec![0]
        } else {
            params
                .split([';', ':'])
                .map(|part| part.parse().unwrap_or(0))
                .collect()
        };
        let mut index = 0;
        while index < values.len() {
            let value = values[index];
            index += 1;
            match value {
                0 => *self = Style::default(),
                1 => self.bold = true,
                2 => self.dim = true,
                3 => self.italic = true,
                4 => self.underline = true,
                7 => self.reverse = true,
                22 => {
                    self.bold = false;
                    self.dim = false;
                }
                23 => self.italic = false,
                24 => self.underline = false,
                27 => self.reverse = false,
                30..=37 => self.fg = Color::Indexed((value - 30) as u8),
                40..=47 => self.bg = Color::Indexed((value - 40) as u8),
                90..=97 => self.fg = Color::Indexed((value - 90 + 8) as u8),
                100..=107 => self.bg = Color::Indexed((value - 100 + 8) as u8),
                39 => self.fg = Color::Default,
                49 => self.bg = Color::Default,
                38 | 48 => {
                    let color = match values.get(index) {
                        Some(2) if index + 3 < values.len() => {
                            let color = Color::Rgb(
                                values[index + 1] as u8,
                                values[index + 2] as u8,
                                values[index + 3] as u8,
                            );
                            index += 4;
                            Some(color)
                        }
                        Some(5) if index + 1 < values.len() => {
                            let color = Color::Indexed(values[index + 1] as u8);
                            index += 2;
                            Some(color)
                        }
                        _ => None,
                    };
                    if let Some(color) = color {
                        if value == 38 {
                            self.fg = color;
                        } else {
                            self.bg = color;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn write_sgr(&self, out: &mut String) {
        use std::fmt::Write as _;
        out.push_str("\x1b[0");
        for (on, code) in [
            (self.bold, ";1"),
            (self.dim, ";2"),
            (self.italic, ";3"),
            (self.underline, ";4"),
            (self.reverse, ";7"),
        ] {
            if on {
                out.push_str(code);
            }
        }
        for (color, base) in [(self.fg, 38), (self.bg, 48)] {
            match color {
                Color::Default => {}
                Color::Indexed(index) => {
                    let _ = write!(out, ";{base};5;{index}");
                }
                Color::Rgb(red, green, blue) => {
                    let _ = write!(out, ";{base};2;{red};{green};{blue}");
                }
            }
        }
        out.push('m');
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Glyph {
    Char(char),
    Cluster(Box<str>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Cell {
    glyph: Glyph,
    /// 1 or 2 for a cell that starts a character, 0 for the right half of a
    /// wide one.
    width: u8,
    style: Style,
}

impl Cell {
    fn blank(style: Style) -> Self {
        Self {
            glyph: Glyph::Char(' '),
            width: 1,
            style,
        }
    }
}

#[derive(Default)]
pub struct FrameDiff {
    size: (usize, usize),
    front: Vec<Cell>,
    back: Vec<Cell>,
}

impl FrameDiff {
    pub fn new() -> Self {
        Self::default()
    }

    /// Turn a frame into the bytes that bring the terminal from the previous
    /// frame to this one. `size` is (columns, rows).
    pub fn render(&mut self, frame: &str, size: (usize, usize)) -> String {
        let (columns, rows) = size;
        let mut out = String::from("\x1b[?2026h\x1b[?25l");
        if size != self.size {
            // After a resize the terminal's contents are unknown: clear it and
            // start from blank, which is what `2J` leaves with default colors.
            self.size = size;
            self.front = vec![Cell::blank(Style::default()); columns * rows];
            out.push_str("\x1b[0m\x1b[2J");
        }
        self.back.clone_from(&self.front);
        let cursor = self.play(frame);

        let mut pen: Option<(usize, usize)> = None;
        let mut pen_style: Option<Style> = None;
        for row in 0..rows {
            let line = row * columns;
            let mut column = 0;
            while column < columns {
                if self.back[line + column] == self.front[line + column] {
                    column += 1;
                    continue;
                }
                let start = if self.back[line + column].width == 0 && column > 0 {
                    column - 1
                } else {
                    column
                };
                let cell = &self.back[line + start];
                if pen != Some((row, start)) {
                    use std::fmt::Write as _;
                    let _ = write!(out, "\x1b[{};{}H", row + 1, start + 1);
                }
                if pen_style != Some(cell.style) {
                    cell.style.write_sgr(&mut out);
                    pen_style = Some(cell.style);
                }
                let width = usize::from(cell.width.max(1));
                match (&cell.glyph, cell.width) {
                    (_, 0) => out.push(' '),
                    (Glyph::Char(character), _) => out.push(*character),
                    (Glyph::Cluster(cluster), _) => out.push_str(cluster),
                }
                pen = Some((row, start + width));
                column = start + width;
            }
        }
        std::mem::swap(&mut self.front, &mut self.back);

        out.push_str("\x1b[0m");
        if let Some((row, column)) = cursor {
            use std::fmt::Write as _;
            let _ = write!(
                out,
                "\x1b[{};{}H\x1b[?25h",
                row.min(rows.saturating_sub(1)) + 1,
                column.min(columns.saturating_sub(1)) + 1
            );
        }
        out.push_str("\x1b[?2026l");
        out
    }

    /// Paint `frame` onto `back`. Returns where the cursor should show, if the
    /// frame left it visible.
    fn play(&mut self, frame: &str) -> Option<(usize, usize)> {
        let (columns, rows) = self.size;
        let mut row = 0usize;
        let mut column = 0usize;
        let mut style = Style::default();
        let mut cursor = None;
        let mut rest = frame;
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix('\x1b') {
                let Some(body) = after.strip_prefix('[') else {
                    // Some other escape: drop it and the character after.
                    let mut chars = after.chars();
                    chars.next();
                    rest = chars.as_str();
                    continue;
                };
                let Some(end) = body.find(|c: char| ('\x40'..='\x7e').contains(&c)) else {
                    break;
                };
                let params = &body[..end];
                let command = body[end..].chars().next().unwrap_or('\0');
                rest = &body[end + command.len_utf8()..];
                let private = params.starts_with('?');
                let number = |index: usize| -> usize {
                    params
                        .trim_start_matches('?')
                        .split(';')
                        .nth(index)
                        .and_then(|part| part.parse().ok())
                        .unwrap_or(0)
                };
                match (private, command) {
                    (false, 'H' | 'f') => {
                        row = number(0).max(1) - 1;
                        column = number(1).max(1) - 1;
                    }
                    (false, 'm') => style.apply_sgr(params),
                    (false, 'J') if number(0) == 2 => {
                        self.back.fill(Cell::blank(Style {
                            bg: style.bg,
                            ..Style::default()
                        }));
                    }
                    (false, 'K') if row < rows => {
                        let from = match number(0) {
                            0 => column.min(columns),
                            _ => 0,
                        };
                        let to = match number(0) {
                            1 => (column + 1).min(columns),
                            _ => columns,
                        };
                        let blank = Cell::blank(Style {
                            bg: style.bg,
                            ..Style::default()
                        });
                        if from < to {
                            self.split_wide(row, from);
                            if to > 0 {
                                self.split_wide(row, to - 1);
                            }
                            for cell in &mut self.back[row * columns + from..row * columns + to] {
                                *cell = blank.clone();
                            }
                        }
                    }
                    (true, 'h') if number(0) == 25 => cursor = Some((row, column)),
                    (true, 'l') if number(0) == 25 => cursor = None,
                    _ => {}
                }
                continue;
            }
            let end = rest.find('\x1b').unwrap_or(rest.len());
            for grapheme in rest[..end].graphemes(true) {
                match grapheme {
                    "\r" => column = 0,
                    "\n" | "\r\n" => {
                        row += 1;
                        if grapheme == "\r\n" {
                            column = 0;
                        }
                    }
                    "\t" => column = (column / 8 + 1) * 8,
                    _ if grapheme.chars().any(char::is_control) => {}
                    _ => {
                        let width = UnicodeWidthStr::width(grapheme);
                        if width == 0 {
                            continue;
                        }
                        let width = width.min(2);
                        if row < rows && column + width <= columns {
                            self.put(row, column, grapheme, width, style);
                        }
                        column += width;
                    }
                }
            }
            rest = &rest[end..];
        }
        cursor
    }

    fn put(&mut self, row: usize, column: usize, grapheme: &str, width: usize, style: Style) {
        let columns = self.size.0;
        self.split_wide(row, column);
        if width == 2 {
            self.split_wide(row, column + 1);
        }
        let mut chars = grapheme.chars();
        let glyph = match (chars.next(), chars.next()) {
            (Some(character), None) => Glyph::Char(character),
            _ => Glyph::Cluster(grapheme.into()),
        };
        let index = row * columns + column;
        self.back[index] = Cell {
            glyph,
            width: width as u8,
            style,
        };
        if width == 2 {
            self.back[index + 1] = Cell {
                glyph: Glyph::Char(' '),
                width: 0,
                style,
            };
        }
    }

    /// Before overwriting a cell, blank the other half of a wide character it
    /// belongs to, as terminals do.
    fn split_wide(&mut self, row: usize, column: usize) {
        let columns = self.size.0;
        let index = row * columns + column;
        match self.back[index].width {
            0 if column > 0 => {
                let style = self.back[index - 1].style;
                self.back[index - 1] = Cell::blank(style);
            }
            2 if column + 1 < columns => {
                let style = self.back[index + 1].style;
                self.back[index + 1] = Cell::blank(style);
            }
            _ => {}
        }
        if self.back[index].width != 1 {
            let style = self.back[index].style;
            self.back[index] = Cell::blank(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_sync(out: &str) -> &str {
        out.strip_prefix("\x1b[?2026h\x1b[?25l")
            .and_then(|rest| rest.strip_suffix("\x1b[?2026l"))
            .unwrap()
    }

    #[test]
    fn unchanged_frame_writes_no_cells() {
        let mut diff = FrameDiff::new();
        let frame = "\x1b[H\x1b[?25l\x1b[1;1H\x1b[1;38;2;1;2;3;48;2;4;5;6mHello\x1b[0m";
        let first = diff.render(frame, (10, 3));
        assert!(first.contains("Hello"));
        let second = diff.render(frame, (10, 3));
        assert_eq!(strip_sync(&second), "\x1b[0m");
    }

    #[test]
    fn only_changed_cells_are_written() {
        let mut diff = FrameDiff::new();
        diff.render("\x1b[2;1H\x1b[0mabcdef\x1b[0m", (10, 3));
        let out = diff.render("\x1b[2;1H\x1b[0mabXdef\x1b[0m", (10, 3));
        assert_eq!(strip_sync(&out), "\x1b[2;3H\x1b[0mX\x1b[0m");
    }

    #[test]
    fn overpainting_within_a_frame_writes_only_the_result() {
        let mut diff = FrameDiff::new();
        diff.render("\x1b[1;1H\x1b[0m     \x1b[0m", (5, 1));
        let out = diff.render(
            "\x1b[1;1H\x1b[48;2;9;9;9m     \x1b[0m\x1b[1;1H\x1b[0m     \x1b[0m",
            (5, 1),
        );
        assert_eq!(strip_sync(&out), "\x1b[0m");
    }

    #[test]
    fn cursor_is_restored_when_visible() {
        let mut diff = FrameDiff::new();
        let out = diff.render("\x1b[2;4H\x1b[?25h", (10, 3));
        assert!(out.ends_with("\x1b[0m\x1b[2;4H\x1b[?25h\x1b[?2026l"));
    }

    #[test]
    fn wide_characters_take_two_cells() {
        let mut diff = FrameDiff::new();
        diff.render("\x1b[1;1H漢x", (4, 1));
        let out = diff.render("\x1b[1;2Hy", (4, 1));
        // Writing into the right half blanks the left half.
        assert_eq!(strip_sync(&out), "\x1b[1;1H\x1b[0m y\x1b[0m");
    }

    #[test]
    fn accumulated_colors_compare_equal() {
        let mut style = Style::default();
        style.apply_sgr("48;2;16;25;31");
        style.apply_sgr("38;2;1;1;1");
        style.apply_sgr("38;2;2;2;2");
        let mut direct = Style::default();
        direct.apply_sgr("0;38;2;2;2;2;48;2;16;25;31");
        assert_eq!(style, direct);
    }
}
