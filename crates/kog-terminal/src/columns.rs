use std::fs;

use kog_audio::settings::{AppSettings, setting_path};

const TUI_LAYOUT_FILE: &str = "tui-column-layout";

#[derive(Clone, Debug)]
pub struct Column {
    pub id: &'static str,
    pub label: &'static str,
    pub width: usize,
    pub visible: bool,
}

#[derive(Clone, Debug)]
pub struct Columns {
    pub entries: Vec<Column>,
    pub scroll: usize,
}

const DEFAULTS: [(&str, &str, usize, bool); 22] = [
    ("index", "#", 5, true),
    ("star", "★", 3, false),
    ("status", "Status", 7, false),
    ("rating", "Rating", 8, false),
    ("title", "Title", 39, true),
    ("albumartist", "Album Artist", 20, false),
    ("artist", "Artist", 20, true),
    ("composer", "Composer", 18, false),
    ("album", "Album", 28, true),
    ("length", "Length", 8, false),
    ("filesizebytes", "Size (bytes)", 15, false),
    ("filesize", "Size", 11, true),
    ("date", "Year", 6, false),
    ("genre", "Genre", 18, false),
    ("track", "№", 5, false),
    ("playcount", "Plays", 7, false),
    ("path", "Path", 30, false),
    ("filename", "Filename", 24, false),
    ("codec", "Codec", 9, false),
    ("samplerate", "Sample Rate", 13, false),
    ("bitspersample", "Bits", 6, false),
    ("bitrate", "Bitrate", 9, false),
];

impl Columns {
    pub fn default_id(index: usize) -> Option<&'static str> {
        DEFAULTS.get(index).map(|(id, ..)| *id)
    }

    pub fn load(settings: &AppSettings) -> Self {
        let tui = setting_path(TUI_LAYOUT_FILE).and_then(|path| fs::read_to_string(path).ok());
        if let Some(layout) = tui.as_deref().and_then(|value| Self::parse(value, false)) {
            return layout;
        }
        if let Some(layout) = settings
            .playlist_column_layout
            .as_deref()
            .and_then(|value| Self::parse(value, true))
        {
            return layout;
        }
        Self::default()
    }

    pub fn parse(value: &str, qt_pixels: bool) -> Option<Self> {
        let mut entries = Vec::with_capacity(DEFAULTS.len());
        for part in value.trim().split(';') {
            let mut fields = part.split(',');
            let id = fields.next()?.trim();
            let width = fields.next()?.trim().parse::<f64>().ok()?;
            let visible = match fields.next()?.trim() {
                "0" => false,
                "1" => true,
                _ => return None,
            };
            if fields.next().is_some() || !width.is_finite() || width <= 0.0 {
                return None;
            }
            let &(identifier, label, _, _) = DEFAULTS.iter().find(|(name, ..)| *name == id)?;
            if entries.iter().any(|column: &Column| column.id == id) {
                return None;
            }
            let width = if qt_pixels { width / 10.0 } else { width };
            entries.push(Column {
                id: identifier,
                label,
                width: (width.round() as usize).clamp(3, 160),
                visible,
            });
        }
        if entries.is_empty() || !entries.iter().any(|column| column.visible) {
            return None;
        }
        for (default_index, column) in Self::default().entries.into_iter().enumerate() {
            if !entries.iter().any(|saved| saved.id == column.id) {
                entries.insert(default_index.min(entries.len()), column);
            }
        }
        Some(Self { entries, scroll: 0 })
    }

    pub fn save(&self) -> Result<(), String> {
        let path = setting_path(TUI_LAYOUT_FILE).ok_or("No settings directory")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let encoded = self
            .entries
            .iter()
            .map(|column| {
                format!(
                    "{},{},{}",
                    column.id,
                    column.width,
                    u8::from(column.visible)
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        fs::write(path, encoded).map_err(|error| error.to_string())
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|column| column.id == id)
    }

    pub fn toggle(&mut self, index: usize) -> bool {
        if index >= self.entries.len()
            || (self.entries[index].visible
                && self.entries.iter().filter(|c| c.visible).count() == 1)
        {
            return false;
        }
        self.entries[index].visible = !self.entries[index].visible;
        true
    }

    pub fn move_by(&mut self, index: usize, delta: isize) -> Option<usize> {
        if index >= self.entries.len() || !self.entries[index].visible {
            return None;
        }
        let target = if delta < 0 {
            (0..index).rev().find(|&other| self.entries[other].visible)
        } else {
            (index + 1..self.entries.len()).find(|&other| self.entries[other].visible)
        }?;
        self.entries.swap(index, target);
        Some(target)
    }

    pub fn positions(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        let mut x = 0;
        self.entries
            .iter()
            .enumerate()
            .filter_map(move |(index, column)| {
                if !column.visible {
                    return None;
                }
                let start = x;
                x += column.width;
                Some((index, start, column.width))
            })
    }

    pub fn total_width(&self) -> usize {
        self.entries
            .iter()
            .filter(|column| column.visible)
            .map(|column| column.width)
            .sum()
    }

    pub fn scroll_by(&mut self, delta: isize, viewport: usize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(delta)
            .min(self.total_width().saturating_sub(viewport));
    }
}

impl Default for Columns {
    fn default() -> Self {
        Self {
            entries: DEFAULTS
                .into_iter()
                .map(|(id, label, width, visible)| Column {
                    id,
                    label,
                    width,
                    visible,
                })
                .collect(),
            scroll: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qt_layout_preserves_column_order_and_scales_to_cells() {
        let value = DEFAULTS
            .iter()
            .rev()
            .map(|(id, _, _, _)| format!("{id},120,1"))
            .collect::<Vec<_>>()
            .join(";");
        let layout = Columns::parse(&value, true).unwrap();
        assert_eq!(layout.entries[0].id, "bitrate");
        assert_eq!(layout.entries[0].width, 12);
        assert_eq!(layout.entries.len(), 22);
    }

    #[test]
    fn old_layout_gains_size_columns_without_losing_saved_widths() {
        let value = DEFAULTS
            .iter()
            .filter(|(id, ..)| *id != "filesize" && *id != "filesizebytes")
            .map(|(id, _, _, _)| format!("{id},17,1"))
            .collect::<Vec<_>>()
            .join(";");
        let layout = Columns::parse(&value, false).unwrap();
        assert_eq!(layout.entries.len(), 22);
        assert_eq!(layout.entries[0].width, 17);
        assert!(layout.entries[layout.index("filesize").unwrap()].visible);
        assert!(!layout.entries[layout.index("filesizebytes").unwrap()].visible);
    }

    #[test]
    fn malformed_or_duplicate_layout_is_rejected() {
        assert!(Columns::parse("title,40,1;title,40,1", false).is_none());
        let mut layout = Columns::default();
        let visible: Vec<_> = layout
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, column)| column.visible.then_some(index))
            .collect();
        for index in visible.iter().skip(1) {
            assert!(layout.toggle(*index));
        }
        assert!(!layout.toggle(visible[0]));
    }
}
