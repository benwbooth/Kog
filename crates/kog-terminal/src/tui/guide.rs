//! The Kog MML guide in the terminal: chapters on the left, text on the right.
use super::{Key, Surface, paint};
use kog_audio::inspection::guide::CHAPTERS;

#[derive(Default)]
pub(super) struct Guide {
    pub open: bool,
    chapter: usize,
    scroll: usize,
}

/// Plain-text lines for a chapter: headings marked for emphasis, code kept
/// as is, inline Markdown markers removed, paragraphs wrapped to `width`.
fn layout(markdown: &str, width: usize) -> Vec<(String, Surface, bool)> {
    let width = width.max(20);
    let mut lines = Vec::new();
    let mut code = false;
    let mut paragraph = String::new();
    let flush = |paragraph: &mut String, lines: &mut Vec<(String, Surface, bool)>| {
        if paragraph.is_empty() {
            return;
        }
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
                lines.push((std::mem::take(&mut line), Surface::Main, false));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push((line, Surface::Main, false));
        }
        paragraph.clear();
    };
    let plain = |text: &str| text.replace("**", "").replace('`', "").replace("\\|", "|");
    for raw in markdown.lines() {
        if raw.trim_start().starts_with("```") {
            flush(&mut paragraph, &mut lines);
            code = !code;
            continue;
        }
        if code {
            lines.push((format!("  {raw}"), Surface::Accent, false));
            continue;
        }
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            flush(&mut paragraph, &mut lines);
            lines.push((String::new(), Surface::Main, false));
        } else if let Some(heading) = trimmed.strip_prefix("# ") {
            flush(&mut paragraph, &mut lines);
            lines.push((plain(heading).to_uppercase(), Surface::Header, true));
        } else if let Some(heading) = trimmed.strip_prefix("## ") {
            flush(&mut paragraph, &mut lines);
            lines.push((plain(heading), Surface::Header, true));
        } else if trimmed.starts_with('|') {
            flush(&mut paragraph, &mut lines);
            if !trimmed.starts_with("| ---") {
                lines.push((plain(trimmed), Surface::Main, false));
            }
        } else if trimmed.starts_with("- ") || trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) && trimmed.contains(". ") && raw == trimmed {
            flush(&mut paragraph, &mut lines);
            paragraph.push_str(&plain(trimmed));
        } else {
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(&plain(trimmed));
        }
    }
    flush(&mut paragraph, &mut lines);
    lines
}

impl Guide {
    pub fn key(&mut self, key: Key) {
        match key {
            Key::Esc | Key::Char('q') | Key::Char('g') => self.open = false,
            Key::Left | Key::Char('[') => {
                self.chapter = self.chapter.saturating_sub(1);
                self.scroll = 0;
            }
            Key::Right | Key::Char(']') => {
                self.chapter = (self.chapter + 1).min(CHAPTERS.len() - 1);
                self.scroll = 0;
            }
            Key::Up => self.scroll = self.scroll.saturating_sub(1),
            Key::Down => self.scroll += 1,
            Key::PageUp => self.scroll = self.scroll.saturating_sub(16),
            Key::PageDown | Key::Char(' ') => self.scroll += 16,
            Key::Home => self.scroll = 0,
            _ => {}
        }
    }

    pub fn draw(&mut self, out: &mut String, size: (usize, usize)) {
        let (width, height) = size;
        for y in 1..=height {
            paint(out, y, 1, &" ".repeat(width), width, Surface::Main, false);
        }
        paint(out, 1, 2, "Kog MML Guide", width - 3, Surface::Accent, true);
        let list = if width >= 70 { 32 } else { 0 };
        if list > 0 {
            for (index, chapter) in CHAPTERS.iter().enumerate().take(height.saturating_sub(4)) {
                let surface = if index == self.chapter { Surface::Selected } else { Surface::Muted };
                paint(out, 3 + index, 2, chapter.title, list - 2, surface, index == self.chapter);
            }
        }
        let x = 2 + list;
        let text_width = width.saturating_sub(x + 2);
        let lines = layout(CHAPTERS[self.chapter].markdown, text_width);
        let rows = height.saturating_sub(4);
        self.scroll = self.scroll.min(lines.len().saturating_sub(rows));
        for (n, (line, surface, bold)) in lines.iter().skip(self.scroll).take(rows).enumerate() {
            paint(out, 3 + n, x, line, text_width, *surface, *bold);
        }
        paint(
            out,
            height,
            2,
            &format!(
                "Chapter {} of {} · ←/→ chapters · ↑↓ PgUp/PgDn scroll · G or Esc close",
                self.chapter + 1,
                CHAPTERS.len()
            ),
            width - 3,
            Surface::Muted,
            false,
        );
    }
}
