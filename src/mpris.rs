//! Linux MPRIS2 integration for desktop media controls and hardware media keys.

use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MprisPlaybackStatus {
    Playing,
    Paused,
    #[default]
    Stopped,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MprisLoopStatus {
    #[default]
    None,
    Track,
    Playlist,
}

#[cfg_attr(test, allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
pub enum MprisCommand {
    Raise,
    Next,
    Previous,
    Pause,
    PlayPause,
    Stop,
    Play,
    SeekBy(f64),
    SetPosition { track_id: String, seconds: f64 },
    OpenUri(String),
    SetLoopStatus(MprisLoopStatus),
    SetShuffle(bool),
    SetVolume(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MprisSnapshot {
    pub playback_status: MprisPlaybackStatus,
    pub loop_status: MprisLoopStatus,
    pub shuffle: bool,
    pub volume: f64,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub track_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub genre: String,
    pub composer: String,
    pub year: Option<u32>,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub url: Option<String>,
    pub can_go_next: bool,
    pub can_go_previous: bool,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_seek: bool,
}

impl Default for MprisSnapshot {
    fn default() -> Self {
        Self {
            playback_status: MprisPlaybackStatus::Stopped,
            loop_status: MprisLoopStatus::None,
            shuffle: false,
            volume: 1.0,
            position_seconds: 0.0,
            duration_seconds: 0.0,
            track_id: None,
            title: String::new(),
            artist: String::new(),
            album_artist: String::new(),
            album: String::new(),
            genre: String::new(),
            composer: String::new(),
            year: None,
            disc_number: None,
            track_number: None,
            url: None,
            can_go_next: false,
            can_go_previous: false,
            can_play: false,
            can_pause: false,
            can_seek: false,
        }
    }
}

#[cfg(all(target_os = "linux", not(test)))]
enum MprisUpdate {
    Snapshot(MprisSnapshot),
    Seeked(f64),
}

pub struct MprisService {
    command_receiver: Receiver<MprisCommand>,
    #[cfg(all(target_os = "linux", not(test)))]
    update_sender: Option<async_channel::Sender<MprisUpdate>>,
}

impl Default for MprisService {
    fn default() -> Self {
        let (command_sender, command_receiver) = mpsc::sync_channel(32);

        #[cfg(all(target_os = "linux", not(test)))]
        let update_sender = {
            let (update_sender, update_receiver) = async_channel::bounded(32);
            let spawn_result = std::thread::Builder::new()
                .name("kog-mpris".to_owned())
                .spawn(move || {
                    if let Err(error) = futures_lite::future::block_on(run_mpris_service(
                        update_receiver,
                        command_sender,
                    )) {
                        eprintln!("Kog MPRIS service stopped: {error}");
                    }
                });
            if let Err(error) = spawn_result {
                eprintln!("Kog could not start the MPRIS service: {error}");
                None
            } else {
                Some(update_sender)
            }
        };

        #[cfg(any(not(target_os = "linux"), test))]
        drop(command_sender);

        Self {
            command_receiver,
            #[cfg(all(target_os = "linux", not(test)))]
            update_sender,
        }
    }
}

impl MprisService {
    pub fn publish(&self, snapshot: MprisSnapshot) {
        #[cfg(all(target_os = "linux", not(test)))]
        if let Some(sender) = &self.update_sender {
            let _ = sender.try_send(MprisUpdate::Snapshot(snapshot));
        }

        #[cfg(any(not(target_os = "linux"), test))]
        let _ = snapshot;
    }

    pub fn seeked(&self, position_seconds: f64) {
        #[cfg(all(target_os = "linux", not(test)))]
        if let Some(sender) = &self.update_sender {
            let _ = sender.try_send(MprisUpdate::Seeked(position_seconds));
        }

        #[cfg(any(not(target_os = "linux"), test))]
        let _ = position_seconds;
    }

    pub fn try_command(&self) -> Option<MprisCommand> {
        match self.command_receiver.try_recv() {
            Ok(command) => Some(command),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

#[cfg(all(target_os = "linux", not(test)))]
async fn run_mpris_service(
    update_receiver: async_channel::Receiver<MprisUpdate>,
    command_sender: std::sync::mpsc::SyncSender<MprisCommand>,
) -> mpris_server::zbus::Result<()> {
    use mpris_server::Player;

    let initial = MprisSnapshot::default();
    let player = Player::builder("kog")
        .identity("Kog")
        .desktop_entry("org.kog.player")
        .supported_uri_schemes(["file", "http", "https"])
        .supported_mime_types([
            "audio/mpeg",
            "audio/flac",
            "audio/x-wav",
            "audio/ogg",
            "audio/x-midi",
            "audio/x-mod",
            "application/x-mpegURL",
            "application/vnd.apple.mpegurl",
        ])
        .can_raise(true)
        .playback_status(server_playback_status(initial.playback_status))
        .loop_status(server_loop_status(initial.loop_status))
        .shuffle(initial.shuffle)
        .metadata(server_metadata(&initial))
        .volume(initial.volume)
        .position(server_time(initial.position_seconds))
        .can_go_next(initial.can_go_next)
        .can_go_previous(initial.can_go_previous)
        .can_play(initial.can_play)
        .can_pause(initial.can_pause)
        .can_seek(initial.can_seek)
        .can_control(true)
        .build()
        .await?;

    connect_commands(&player, command_sender);

    let run_task = player.run();
    let update_task = process_updates(&player, update_receiver, initial);
    futures_lite::future::race(run_task, update_task).await;
    Ok(())
}

#[cfg(all(target_os = "linux", not(test)))]
fn connect_commands(
    player: &mpris_server::Player,
    command_sender: std::sync::mpsc::SyncSender<MprisCommand>,
) {
    let sender = command_sender.clone();
    player.connect_raise(move |_| queue_command(&sender, MprisCommand::Raise));

    let sender = command_sender.clone();
    player.connect_next(move |_| queue_command(&sender, MprisCommand::Next));

    let sender = command_sender.clone();
    player.connect_previous(move |_| queue_command(&sender, MprisCommand::Previous));

    let sender = command_sender.clone();
    player.connect_pause(move |_| queue_command(&sender, MprisCommand::Pause));

    let sender = command_sender.clone();
    player.connect_play_pause(move |_| queue_command(&sender, MprisCommand::PlayPause));

    let sender = command_sender.clone();
    player.connect_stop(move |_| queue_command(&sender, MprisCommand::Stop));

    let sender = command_sender.clone();
    player.connect_play(move |_| queue_command(&sender, MprisCommand::Play));

    let sender = command_sender.clone();
    player.connect_seek(move |_, offset| {
        queue_command(
            &sender,
            MprisCommand::SeekBy(micros_to_seconds(offset.as_micros())),
        )
    });

    let sender = command_sender.clone();
    player.connect_set_position(move |_, track_id, position| {
        queue_command(
            &sender,
            MprisCommand::SetPosition {
                track_id: track_id.to_string(),
                seconds: micros_to_seconds(position.as_micros()),
            },
        )
    });

    let sender = command_sender.clone();
    player.connect_open_uri(move |_, uri| {
        queue_command(&sender, MprisCommand::OpenUri(uri.to_owned()))
    });

    let sender = command_sender.clone();
    player.connect_set_loop_status(move |_, status| {
        queue_command(
            &sender,
            MprisCommand::SetLoopStatus(match status {
                mpris_server::LoopStatus::None => MprisLoopStatus::None,
                mpris_server::LoopStatus::Track => MprisLoopStatus::Track,
                mpris_server::LoopStatus::Playlist => MprisLoopStatus::Playlist,
            }),
        )
    });

    let sender = command_sender.clone();
    player.connect_set_shuffle(move |_, shuffle| {
        queue_command(&sender, MprisCommand::SetShuffle(shuffle))
    });

    player.connect_set_volume(move |_, volume| {
        queue_command(&command_sender, MprisCommand::SetVolume(volume))
    });
}

#[cfg(all(target_os = "linux", not(test)))]
fn queue_command(sender: &std::sync::mpsc::SyncSender<MprisCommand>, command: MprisCommand) {
    if let Err(error) = sender.try_send(command) {
        eprintln!("Kog dropped an MPRIS command because its queue is unavailable: {error}");
    }
}

#[cfg(all(target_os = "linux", not(test)))]
async fn process_updates(
    player: &mpris_server::Player,
    update_receiver: async_channel::Receiver<MprisUpdate>,
    mut previous: MprisSnapshot,
) {
    while let Ok(update) = update_receiver.recv().await {
        match update {
            MprisUpdate::Snapshot(snapshot) => {
                apply_snapshot(player, &previous, &snapshot).await;
                previous = snapshot;
            }
            MprisUpdate::Seeked(position_seconds) => {
                if let Err(error) = player.seeked(server_time(position_seconds)).await {
                    eprintln!("Kog could not publish the MPRIS seek position: {error}");
                }
            }
        }
    }
}

#[cfg(all(target_os = "linux", not(test)))]
async fn apply_snapshot(
    player: &mpris_server::Player,
    previous: &MprisSnapshot,
    snapshot: &MprisSnapshot,
) {
    if previous.playback_status != snapshot.playback_status
        && let Err(error) = player
            .set_playback_status(server_playback_status(snapshot.playback_status))
            .await
    {
        eprintln!("Kog could not update MPRIS playback state: {error}");
    }
    if previous.loop_status != snapshot.loop_status
        && let Err(error) = player
            .set_loop_status(server_loop_status(snapshot.loop_status))
            .await
    {
        eprintln!("Kog could not update MPRIS repeat state: {error}");
    }
    if previous.shuffle != snapshot.shuffle
        && let Err(error) = player.set_shuffle(snapshot.shuffle).await
    {
        eprintln!("Kog could not update MPRIS shuffle state: {error}");
    }
    if previous.volume != snapshot.volume
        && let Err(error) = player.set_volume(snapshot.volume).await
    {
        eprintln!("Kog could not update MPRIS volume: {error}");
    }
    if metadata_changed(previous, snapshot)
        && let Err(error) = player.set_metadata(server_metadata(snapshot)).await
    {
        eprintln!("Kog could not update MPRIS metadata: {error}");
    }
    if previous.can_go_next != snapshot.can_go_next
        && let Err(error) = player.set_can_go_next(snapshot.can_go_next).await
    {
        eprintln!("Kog could not update MPRIS Next availability: {error}");
    }
    if previous.can_go_previous != snapshot.can_go_previous
        && let Err(error) = player.set_can_go_previous(snapshot.can_go_previous).await
    {
        eprintln!("Kog could not update MPRIS Previous availability: {error}");
    }
    if previous.can_play != snapshot.can_play
        && let Err(error) = player.set_can_play(snapshot.can_play).await
    {
        eprintln!("Kog could not update MPRIS Play availability: {error}");
    }
    if previous.can_pause != snapshot.can_pause
        && let Err(error) = player.set_can_pause(snapshot.can_pause).await
    {
        eprintln!("Kog could not update MPRIS Pause availability: {error}");
    }
    if previous.can_seek != snapshot.can_seek
        && let Err(error) = player.set_can_seek(snapshot.can_seek).await
    {
        eprintln!("Kog could not update MPRIS seek availability: {error}");
    }

    // The MPRIS specification explicitly forbids PropertiesChanged for Position.
    player.set_position(server_time(snapshot.position_seconds));
}

fn metadata_changed(previous: &MprisSnapshot, snapshot: &MprisSnapshot) -> bool {
    previous.duration_seconds != snapshot.duration_seconds
        || previous.track_id != snapshot.track_id
        || previous.title != snapshot.title
        || previous.artist != snapshot.artist
        || previous.album_artist != snapshot.album_artist
        || previous.album != snapshot.album
        || previous.genre != snapshot.genre
        || previous.composer != snapshot.composer
        || previous.year != snapshot.year
        || previous.disc_number != snapshot.disc_number
        || previous.track_number != snapshot.track_number
        || previous.url != snapshot.url
}

#[cfg(target_os = "linux")]
fn server_metadata(snapshot: &MprisSnapshot) -> mpris_server::Metadata {
    use mpris_server::{Metadata, TrackId};

    let track_id = snapshot
        .track_id
        .as_deref()
        .and_then(|track_id| TrackId::try_from(track_id).ok())
        .unwrap_or(TrackId::NO_TRACK);
    let mut builder = Metadata::builder().trackid(track_id);
    if snapshot.duration_seconds > 0.0 {
        builder = builder.length(server_time(snapshot.duration_seconds));
    }
    if !snapshot.title.is_empty() {
        builder = builder.title(snapshot.title.clone());
    }
    if !snapshot.artist.is_empty() {
        builder = builder.artist([snapshot.artist.clone()]);
    }
    if !snapshot.album_artist.is_empty() {
        builder = builder.album_artist([snapshot.album_artist.clone()]);
    }
    if !snapshot.album.is_empty() {
        builder = builder.album(snapshot.album.clone());
    }
    if !snapshot.genre.is_empty() {
        builder = builder.genre([snapshot.genre.clone()]);
    }
    if !snapshot.composer.is_empty() {
        builder = builder.composer([snapshot.composer.clone()]);
    }
    if let Some(year) = snapshot.year {
        builder = builder.content_created(format!("{year:04}-01-01T00:00:00Z"));
    }
    if let Some(disc_number) = snapshot.disc_number {
        builder = builder.disc_number(i32::try_from(disc_number).unwrap_or(i32::MAX));
    }
    if let Some(track_number) = snapshot.track_number {
        builder = builder.track_number(i32::try_from(track_number).unwrap_or(i32::MAX));
    }
    if let Some(url) = snapshot.url.as_ref().filter(|url| !url.is_empty()) {
        builder = builder.url(url.clone());
    }
    builder.build()
}

#[cfg(all(target_os = "linux", not(test)))]
fn server_playback_status(status: MprisPlaybackStatus) -> mpris_server::PlaybackStatus {
    match status {
        MprisPlaybackStatus::Playing => mpris_server::PlaybackStatus::Playing,
        MprisPlaybackStatus::Paused => mpris_server::PlaybackStatus::Paused,
        MprisPlaybackStatus::Stopped => mpris_server::PlaybackStatus::Stopped,
    }
}

#[cfg(all(target_os = "linux", not(test)))]
fn server_loop_status(status: MprisLoopStatus) -> mpris_server::LoopStatus {
    match status {
        MprisLoopStatus::None => mpris_server::LoopStatus::None,
        MprisLoopStatus::Track => mpris_server::LoopStatus::Track,
        MprisLoopStatus::Playlist => mpris_server::LoopStatus::Playlist,
    }
}

#[cfg(target_os = "linux")]
fn server_time(seconds: f64) -> mpris_server::Time {
    mpris_server::Time::from_micros(seconds_to_micros(seconds))
}

fn seconds_to_micros(seconds: f64) -> i64 {
    if !seconds.is_finite() {
        return 0;
    }
    (seconds * 1_000_000.0).clamp(i64::MIN as f64, i64::MAX as f64) as i64
}

fn micros_to_seconds(micros: i64) -> f64 {
    micros as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_change_detection_ignores_regular_position_progress() {
        let before = MprisSnapshot {
            title: "Song".to_owned(),
            position_seconds: 1.0,
            ..MprisSnapshot::default()
        };
        let after = MprisSnapshot {
            position_seconds: 2.0,
            ..before.clone()
        };
        assert!(!metadata_changed(&before, &after));
    }

    #[test]
    fn time_conversion_preserves_signed_seek_offsets() {
        assert_eq!(seconds_to_micros(1.25), 1_250_000);
        assert_eq!(seconds_to_micros(-0.5), -500_000);
        assert_eq!(micros_to_seconds(-750_000), -0.75);
        assert_eq!(seconds_to_micros(f64::NAN), 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn metadata_contains_required_track_identity_and_user_fields() {
        let snapshot = MprisSnapshot {
            track_id: Some("/org/kog/player/track/7".to_owned()),
            title: "Duck Theme".to_owned(),
            artist: "Composer".to_owned(),
            duration_seconds: 12.5,
            ..MprisSnapshot::default()
        };
        let metadata = server_metadata(&snapshot);
        assert_eq!(
            metadata.trackid().unwrap().to_string(),
            snapshot.track_id.unwrap()
        );
        assert_eq!(metadata.title(), Some("Duck Theme"));
        assert_eq!(metadata.artist(), Some(vec!["Composer".to_owned()]));
        assert_eq!(metadata.length().unwrap().as_micros(), 12_500_000);
    }
}
