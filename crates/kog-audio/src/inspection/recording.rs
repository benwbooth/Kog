//! Timed channel metadata produced by the decoder that encoded the audio.
//! Each independently compressed window can be read while encoding continues.
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

use super::{Monitor, Snapshot};
use crate::streaming::PcmReader;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use kog_inspection::{Delta, FrameData, Row, TimedFrame, Window};

const MAGIC: &[u8; 8] = b"KOGCHN01";
const MAX_BLOCK: u32 = 16 * 1024 * 1024;

pub struct CaptureReader {
    pcm: PcmReader,
    monitor: Monitor,
    recording: Option<Recording>,
    start: f64,
    bytes: u64,
    bytes_per_second: f64,
    last_frame: Option<f64>,
}

impl CaptureReader {
    pub fn new(pcm: PcmReader, file: File, start_ms: u64) -> Result<Self, String> {
        let monitor = pcm.inspection_monitor();
        let start = start_ms as f64 / 1000.0;
        let snapshot = monitor.snapshot(Duration::from_secs_f64(start), true, false);
        let bytes_per_second = f64::from(pcm.sample_rate()) * f64::from(pcm.channels()) * 4.0;
        let recording = Recording::new(file, start, snapshot)?;
        Ok(Self {
            pcm,
            monitor,
            recording: Some(recording),
            start,
            bytes: 0,
            bytes_per_second,
            last_frame: (start_ms > 0).then_some(start - 0.000_001),
        })
    }

    pub fn duration(&self) -> Option<Duration> {
        self.pcm.duration()
    }

    pub fn seek(&mut self, position: Duration) -> Result<(), String> {
        self.pcm.seek(position)?;
        self.start = position.as_secs_f64();
        self.bytes = 0;
        self.last_frame = Some(self.start - 0.000_001);
        if let Some(recording) = &mut self.recording {
            recording.file.set_len(0).map_err(|e| e.to_string())?;
            recording
                .file
                .seek(SeekFrom::Start(0))
                .map_err(|e| e.to_string())?;
            let file = recording.file.try_clone().map_err(|e| e.to_string())?;
            *recording = Recording::new(
                file,
                self.start,
                self.monitor.snapshot(position, true, false),
            )?;
        }
        Ok(())
    }

    /// Native players can buffer much farther ahead than the in-memory feed.
    /// Their private recording preserves the consumed position without changing
    /// the platform player's buffering policy.
    pub fn channel_snapshot(&self, path: &Path, position: Duration, playing: bool) -> Snapshot {
        let seconds = position.as_secs_f64();
        if let Some(recording) = &self.recording {
            if seconds >= recording.window.start && seconds < recording.window.end {
                return recording.window.snapshot(seconds, playing, false);
            }
        }
        if let Ok(Some(window)) = read_window(path, seconds) {
            return window.snapshot(seconds, playing, false);
        }
        self.monitor.snapshot(position, playing, false)
    }

    fn capture(&mut self, end: bool) {
        let Some(recording) = &mut self.recording else {
            return;
        };
        let position = self.start + self.bytes as f64 / self.bytes_per_second;
        let result = (|| {
            for frame in self
                .monitor
                .recording_frames(self.last_frame, Duration::from_secs_f64(position))
            {
                self.last_frame = Some(frame.time);
                recording.push(frame)?;
            }
            recording.advance(position)?;
            if end {
                recording.finish(position)?;
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            eprintln!("Kog: channel recording stopped: {error}");
            self.recording = None;
        }
    }
}

impl Read for CaptureReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let count = self.pcm.read(output)?;
        self.bytes = self.bytes.saturating_add(count as u64);
        self.capture(count == 0);
        Ok(count)
    }
}

struct Recording {
    file: File,
    window: Window,
    current: FrameData,
    rows: VecDeque<Row>,
    last_row: Option<Row>,
    finished: bool,
}

impl Recording {
    fn new(mut file: File, start: f64, snapshot: Snapshot) -> Result<Self, String> {
        file.write_all(MAGIC).map_err(|e| e.to_string())?;
        let current = FrameData {
            channels: snapshot.channels,
            global: snapshot.global,
            row: None,
        };
        let rows = snapshot
            .rows
            .into_iter()
            .filter(|row| row.time <= start)
            .collect::<VecDeque<_>>();
        let window = Window {
            version: 1,
            description: snapshot.description,
            start,
            end: start + 1.0,
            initial: current.clone(),
            rows: rows.iter().cloned().collect(),
            frames: Vec::new(),
        };
        Ok(Self {
            file,
            window,
            current,
            last_row: rows.back().cloned(),
            rows,
            finished: false,
        })
    }

    fn push(&mut self, frame: TimedFrame) -> Result<(), String> {
        if frame.time < self.window.start {
            return Ok(());
        }
        self.advance(frame.time)?;
        let mut delta = Delta::between(&self.current, &frame);
        if delta.row.as_ref().is_some_and(|row| {
            self.last_row.as_ref().is_some_and(|last| {
                last.label == row.label && last.cells == row.cells && last.global == row.global
            })
        }) {
            delta.row = None;
        }
        if let Some(row) = &mut delta.row {
            row.time = frame.time;
            self.last_row = Some(row.clone());
            self.rows.push_back(row.clone());
            while self.rows.len() > 24 {
                self.rows.pop_front();
            }
        }
        self.current = frame.data;
        if !delta.is_empty() {
            self.window.frames.push(delta);
        }
        Ok(())
    }

    fn advance(&mut self, position: f64) -> Result<(), String> {
        while position >= self.window.end && !self.finished {
            self.write_window()?;
            self.window = Window {
                version: 1,
                description: self.window.description.clone(),
                start: self.window.end,
                end: self.window.end + 1.0,
                initial: self.current.clone(),
                rows: self.rows.iter().cloned().collect(),
                frames: Vec::new(),
            };
        }
        Ok(())
    }

    fn finish(&mut self, position: f64) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        // Keep the final state available while paused at the end of a track.
        self.window.end = position.max(self.window.start) + 1.0;
        self.write_window()?;
        self.finished = true;
        Ok(())
    }

    fn write_window(&mut self) -> Result<(), String> {
        let mut zip = GzEncoder::new(Vec::new(), Compression::fast());
        serde_json::to_writer(&mut zip, &self.window).map_err(|e| e.to_string())?;
        let bytes = zip.finish().map_err(|e| e.to_string())?;
        let len = u32::try_from(bytes.len()).map_err(|_| "Channel window is too large")?;
        if len > MAX_BLOCK {
            return Err("Channel window exceeds the recording limit".into());
        }
        self.file
            .write_all(&self.window.start.to_le_bytes())
            .map_err(|e| e.to_string())?;
        self.file
            .write_all(&self.window.end.to_le_bytes())
            .map_err(|e| e.to_string())?;
        self.file
            .write_all(&len.to_le_bytes())
            .map_err(|e| e.to_string())?;
        self.file.write_all(&bytes).map_err(|e| e.to_string())?;
        self.file.flush().map_err(|e| e.to_string())
    }
}

/// Missing/incomplete windows are pending, not fabricated empty channel data.
pub fn read_window(path: &Path, position: f64) -> Result<Option<Window>, String> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut magic = [0; 8];
    if file.read_exact(&mut magic).is_err() {
        return Ok(None);
    }
    if &magic != MAGIC {
        return Err("Unrecognized channel recording".into());
    }
    let total = file.metadata().map_err(|e| e.to_string())?.len();
    loop {
        let mut header = [0; 20];
        if file.read_exact(&mut header).is_err() {
            return Ok(None);
        }
        let start = f64::from_le_bytes(header[..8].try_into().unwrap());
        let end = f64::from_le_bytes(header[8..16].try_into().unwrap());
        let len = u32::from_le_bytes(header[16..].try_into().unwrap());
        if !start.is_finite() || !end.is_finite() || end <= start || len > MAX_BLOCK {
            return Err("Invalid channel recording".into());
        }
        if file.stream_position().map_err(|e| e.to_string())? + u64::from(len) > total {
            return Ok(None);
        }
        if position >= start && position < end {
            let mut bytes = vec![0; len as usize];
            file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
            let zip = GzDecoder::new(bytes.as_slice());
            let window = serde_json::from_reader(zip.take(64 * 1024 * 1024))
                .map_err(|e| format!("Reading channel window: {e}"))?;
            return Ok(Some(window));
        }
        if start > position {
            return Ok(None);
        }
        file.seek(SeekFrom::Current(i64::from(len)))
            .map_err(|e| e.to_string())?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kog_inspection::{Channel, Note};
    #[test]
    fn recording_keeps_fast_events_and_nonzero_seek_timestamps() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("track.channels");
        let file = File::create(&path).unwrap();
        let mut recording = Recording::new(file, 10.5, Snapshot::default()).unwrap();
        for (time, key) in [(10.505, 60.0), (10.510, 64.0), (11.7, 67.0)] {
            recording
                .push(TimedFrame {
                    time,
                    data: FrameData {
                        channels: vec![Channel {
                            id: 1,
                            active: true,
                            notes: vec![Note {
                                key,
                                velocity: 1.0,
                                held: true,
                            }],
                            ..Channel::default()
                        }],
                        ..FrameData::default()
                    },
                })
                .unwrap();
        }
        recording.finish(12.0).unwrap();
        for (time, key) in [(10.507, 60.0), (10.511, 64.0), (11.6, 64.0), (11.8, 67.0)] {
            let window = read_window(&path, time).unwrap().unwrap();
            assert_eq!(
                window.snapshot(time, false, false).channels[0].notes[0].key,
                key
            );
        }
        assert!(read_window(&path, 9.0).unwrap().is_none());
    }
}
