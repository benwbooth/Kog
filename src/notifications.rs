//! Cross-platform now-playing notifications with transport actions.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackNotificationAction {
    Previous,
    PlayPause,
    Next,
}

pub struct TrackNotificationService {
    action_sender: Sender<PlaybackNotificationAction>,
    action_receiver: Receiver<PlaybackNotificationAction>,
}

impl Default for TrackNotificationService {
    fn default() -> Self {
        let (action_sender, action_receiver) = mpsc::channel();
        Self {
            action_sender,
            action_receiver,
        }
    }
}

impl TrackNotificationService {
    pub fn show(&self, title: &str, artist: &str) {
        let title = escape_notification_markup(title);
        let artist = escape_notification_markup(artist);
        let action_sender = self.action_sender.clone();
        let spawn_result = std::thread::Builder::new()
            .name("kog-track-notification".to_owned())
            .spawn(move || {
                let body = if artist.trim().is_empty() {
                    "Now playing".to_owned()
                } else {
                    artist
                };
                let handle = notify_rust::Notification::new()
                    .appname("Kog")
                    .summary(&title)
                    .body(&body)
                    .icon("org.kog.player")
                    .action("previous", "Previous")
                    .action("play-pause", "Pause")
                    .action("next", "Next")
                    .timeout(8_000)
                    .show();
                let handle = match handle {
                    Ok(handle) => handle,
                    Err(error) => {
                        eprintln!("Kog could not show the track notification: {error}");
                        return;
                    }
                };
                handle.wait_for_action(move |action| {
                    if let Some(action) = action_from_identifier(action) {
                        let _ = action_sender.send(action);
                    }
                });
            });
        if let Err(error) = spawn_result {
            eprintln!("Kog could not start the track notification worker: {error}");
        }
    }

    pub fn try_action(&self) -> Option<PlaybackNotificationAction> {
        match self.action_receiver.try_recv() {
            Ok(action) => Some(action),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

fn escape_notification_markup(text: &str) -> String {
    // Freedesktop notification bodies may be rendered as lightweight markup.
    // Treat track metadata as text so titles cannot accidentally alter it.
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn action_from_identifier(identifier: &str) -> Option<PlaybackNotificationAction> {
    match identifier {
        "previous" => Some(PlaybackNotificationAction::Previous),
        "play-pause" => Some(PlaybackNotificationAction::PlayPause),
        "next" => Some(PlaybackNotificationAction::Next),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_actions_are_explicit_and_ignore_dismissal() {
        assert_eq!(
            action_from_identifier("previous"),
            Some(PlaybackNotificationAction::Previous)
        );
        assert_eq!(
            action_from_identifier("play-pause"),
            Some(PlaybackNotificationAction::PlayPause)
        );
        assert_eq!(
            action_from_identifier("next"),
            Some(PlaybackNotificationAction::Next)
        );
        assert_eq!(action_from_identifier("default"), None);
        assert_eq!(action_from_identifier("__closed"), None);
    }

    #[test]
    fn metadata_is_escaped_before_notification_rendering() {
        assert_eq!(escape_notification_markup("A & <B>"), "A &amp; &lt;B&gt;");
    }
}
