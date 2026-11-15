//! The pinned Home tab in the terminal: the mixes made for the listener,
//! one line each under their section's heading. Enter opens the mix as a
//! read-only playlist tab; p plays it now.
use super::*;
use kog_audio::playback_order::workspace::{Command, mix_playlist_id, mix_tab_key};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum HomeLine {
    Heading(String),
    Mix { id: String, title: String, subtitle: String, songs: usize },
    Note(String),
}

impl Ui {
    pub(super) fn home_active(&self) -> bool {
        self.workspace_active == kog_audio::playback_order::workspace::HOME_TAB
    }

    /// Take the latest mixes when they changed.
    pub(super) fn refresh_home(&mut self) {
        let discover = kog_server::discover::shared();
        let generation = discover.generation();
        if generation == self.home_generation && !self.home_lines.is_empty() {
            return;
        }
        self.home_generation = generation;
        let mut lines = Vec::new();
        match discover.home() {
            Some(home) if !home.sections.is_empty() => {
                for section in home.sections {
                    lines.push(HomeLine::Heading(section.title));
                    for mix in section.mixes {
                        lines.push(HomeLine::Mix {
                            songs: mix.songs.len(),
                            id: mix.id,
                            title: mix.title,
                            subtitle: mix.subtitle,
                        });
                    }
                }
            }
            _ => {
                let songs = discover.status().songs;
                lines.push(HomeLine::Note(if songs > 0 {
                    format!("Kog is reading your library in the background ({songs} songs so far).")
                } else {
                    "Your mixes appear here once Kog has read some of your library.".to_owned()
                }));
                lines.push(HomeLine::Note("Listening to music helps it learn what you like.".to_owned()));
            }
        }
        self.home_lines = lines;
        if !matches!(self.home_lines.get(self.home_cursor), Some(HomeLine::Mix { .. })) {
            self.home_cursor = self
                .home_lines
                .iter()
                .position(|line| matches!(line, HomeLine::Mix { .. }))
                .unwrap_or(0);
        }
    }

    fn home_mix(&self) -> Option<(String, String)> {
        match self.home_lines.get(self.home_cursor)? {
            HomeLine::Mix { id, title, .. } => Some((id.clone(), title.clone())),
            _ => None,
        }
    }

    fn move_home_cursor(&mut self, delta: isize) {
        let mixes: Vec<usize> = self
            .home_lines
            .iter()
            .enumerate()
            .filter(|(_, line)| matches!(line, HomeLine::Mix { .. }))
            .map(|(index, _)| index)
            .collect();
        if mixes.is_empty() {
            return;
        }
        let current = mixes.iter().position(|index| *index == self.home_cursor).unwrap_or(0);
        let next = (current as isize + delta).clamp(0, mixes.len() as isize - 1) as usize;
        self.home_cursor = mixes[next];
    }

    pub(super) fn open_home_mix(&mut self) {
        let Some((id, title)) = self.home_mix() else { return };
        let Some(playlist_id) = mix_playlist_id(&id) else { return };
        self.workspace_command(Command::Open {
            key: mix_tab_key(&id),
            scope: "local".into(),
            playlist_id,
            name: title,
            readonly: true,
        });
    }

    pub(super) fn play_home_mix(&mut self) {
        let Some((id, _)) = self.home_mix() else { return };
        let Some((_, entries)) = kog_server::discover::shared().mix_entries(&id) else {
            self.status = "That mix has been replaced by newer ones".into();
            return;
        };
        self.session_command(SessionCommand::Expand {
            scope: "local".into(),
            entries,
            action: QueueAction::PlayNow,
        });
    }

    /// Keys for the Home tab while the songs pane has focus.
    pub(super) fn home_key(&mut self, key: Key) -> bool {
        if !self.home_active() || self.focus != Focus::Tracks {
            return false;
        }
        match key {
            Key::Up | Key::Char('k') => self.move_home_cursor(-1),
            Key::Down | Key::Char('j') => self.move_home_cursor(1),
            Key::PageUp => self.move_home_cursor(-10),
            Key::PageDown => self.move_home_cursor(10),
            Key::Home => self.move_home_cursor(-10_000),
            Key::End => self.move_home_cursor(10_000),
            Key::Enter => self.open_home_mix(),
            Key::Char('p') | Key::Char(' ') => self.play_home_mix(),
            Key::Char('R') => {
                kog_server::discover::shared().refresh();
                self.status = "Making new mixes".into();
            }
            _ => return false,
        }
        true
    }

    /// The Home tab where the songs table would be.
    pub(super) fn draw_home(&mut self, screen: &mut String, layout: &Layout, x: usize, width: usize) {
        let top = layout.track_header + 1;
        let bottom = layout.footer_top;
        let rows = bottom.saturating_sub(top);
        if rows == 0 || width == 0 {
            return;
        }
        // Keep the cursor's line in view.
        if self.home_cursor < self.home_offset {
            self.home_offset = self.home_cursor.saturating_sub(1);
        } else if self.home_cursor >= self.home_offset + rows {
            self.home_offset = self.home_cursor + 1 - rows;
        }
        paint(screen, top, x, " Home  ·  Enter opens a mix, p plays it, R makes new mixes", width, Surface::Header, true);
        for row in 1..rows {
            let index = self.home_offset + row - 1;
            let y = top + row;
            match self.home_lines.get(index) {
                Some(HomeLine::Heading(title)) => {
                    paint(screen, y, x, &format!(" {title}"), width, Surface::Main, true);
                }
                Some(HomeLine::Mix { title, subtitle, songs, .. }) => {
                    let selected = index == self.home_cursor;
                    let surface = if selected { Surface::Selected } else { Surface::Main };
                    let count = format!("{songs} songs ");
                    let title_width = (width / 3).clamp(18, 40).min(width);
                    let label = format!("   {}", truncate(title, title_width.saturating_sub(3)));
                    paint(screen, y, x, &label, title_width, surface, selected);
                    let rest = width.saturating_sub(title_width + cell_width(&count));
                    paint(
                        screen,
                        y,
                        x + title_width,
                        subtitle,
                        rest,
                        if selected { Surface::SelectedMuted } else { Surface::MainMuted },
                        false,
                    );
                    paint(
                        screen,
                        y,
                        x + title_width + rest,
                        &count,
                        cell_width(&count),
                        if selected { Surface::SelectedMuted } else { Surface::MainMuted },
                        false,
                    );
                }
                Some(HomeLine::Note(text)) => {
                    paint(screen, y, x, &format!(" {text}"), width, Surface::MainMuted, false);
                }
                None => paint(screen, y, x, "", width, Surface::Main, false),
            }
        }
    }
}
