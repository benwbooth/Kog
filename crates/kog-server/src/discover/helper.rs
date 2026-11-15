//! A helper process for the background reading. Reading a song's tags,
//! measuring it and taking a clip for the sound model all run the song
//! through its decoder, and some decoders (emulators written for playback,
//! not for a million songs in a row) leak memory, hang or crash. So the
//! crawl hands that work to a child process, the same program started with
//! [`HELPER_ARG`], and starts a fresh one every few hundred jobs, when it
//! grows too big, or when a job takes too long. Whatever a decoder leaks
//! goes away with the child.
//!
//! The parent and the child speak one JSON object per line over the child's
//! stdin and stdout.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use base64::Engine;
use kog_audio::decoder::{DecoderRegistry, DecoderSettings, PlaybackSource};
use kog_core::db::StoredEntry;
use serde::{Deserialize, Serialize};

use super::store::Song;

/// The argument that makes a Kog binary act as the helper.
pub const HELPER_ARG: &str = "--kog-discover-helper";

/// Jobs one helper does before a fresh one takes over.
const JOBS_PER_HELPER: usize = 400;
/// A helper using more memory than this is replaced after its job.
const MAX_HELPER_KB: u64 = 1_500_000;

/// A song's address, as it crosses to the helper.
#[derive(Serialize, Deserialize)]
struct Locator {
    kind: String,
    path: String,
    entry: String,
    fragment: Option<String>,
}

impl From<&StoredEntry> for Locator {
    fn from(entry: &StoredEntry) -> Self {
        Self { kind: entry.kind.clone(), path: entry.path.clone(), entry: entry.entry.clone(), fragment: entry.fragment.clone() }
    }
}

impl From<Locator> for StoredEntry {
    fn from(locator: Locator) -> Self {
        Self { kind: locator.kind, path: locator.path, entry: locator.entry, fragment: locator.fragment }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Read { unit: PathBuf, added_at: i64 },
    Measure { entry: Locator },
    Clip { entry: Locator },
}

#[derive(Serialize, Deserialize, Default)]
struct Response {
    #[serde(default)]
    songs: Vec<Song>,
    #[serde(default)]
    features: Option<Vec<f32>>,
    /// A clip's samples, little-endian f32, base64.
    #[serde(default)]
    clip: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// Run as the helper when the program was started as one; returns only
/// when it was not. Call it first thing in every Kog binary's `main`.
pub fn run_if_requested() {
    if std::env::args().nth(1).as_deref() != Some(HELPER_ARG) {
        return;
    }
    super::lower_priority();
    let settings = kog_audio::settings::AppSettings::load().decoder_settings();
    let decoders = DecoderRegistry::new(settings.clone());
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                handle(&decoders, &settings, request)
            }))
            .unwrap_or_else(|_| Response { error: Some("the decoder failed".into()), ..Response::default() }),
            Err(error) => Response { error: Some(error.to_string()), ..Response::default() },
        };
        let Ok(text) = serde_json::to_string(&response) else { break };
        if writeln!(stdout, "{text}").and_then(|()| stdout.flush()).is_err() {
            break;
        }
    }
    std::process::exit(0);
}

fn source_of(decoders: &DecoderRegistry, entry: &StoredEntry) -> Result<PlaybackSource, String> {
    let playlist_entry = kog_audio::playlist::PlaylistEntry::from_locator(
        &entry.kind,
        &entry.path,
        &entry.entry,
        entry.fragment.clone(),
    )?;
    decoders
        .expand_entry(&playlist_entry)?
        .sources
        .into_iter()
        .next()
        .ok_or_else(|| "nothing to play".to_owned())
}

fn handle(decoders: &DecoderRegistry, settings: &DecoderSettings, request: Request) -> Response {
    match request {
        Request::Read { unit, added_at } => Response {
            songs: super::read_unit(decoders, &unit, added_at),
            ..Response::default()
        },
        Request::Measure { entry } => {
            let entry = StoredEntry::from(entry);
            match source_of(decoders, &entry).and_then(|source| super::features::analyze(source, settings)) {
                Ok(features) => Response { features: Some(features), ..Response::default() },
                Err(error) => Response { error: Some(error), ..Response::default() },
            }
        }
        Request::Clip { entry } => {
            let entry = StoredEntry::from(entry);
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                match source_of(decoders, &entry).and_then(|source| super::embedding::read_clip(source, settings)) {
                    Ok(clip) => {
                        let bytes: Vec<u8> = clip.iter().flat_map(|sample| sample.to_le_bytes()).collect();
                        Response {
                            clip: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
                            ..Response::default()
                        }
                    }
                    Err(error) => Response { error: Some(error), ..Response::default() },
                }
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                let _ = entry;
                Response { error: Some("no sound model here".into()), ..Response::default() }
            }
        }
    }
}

/// The parent's side: one running helper, replaced as needed.
#[derive(Default)]
pub struct Helper {
    running: Option<Running>,
}

struct Running {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    jobs: usize,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn resident_kb(pid: u32) -> u64 {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with("VmRSS:"))?
                .split_whitespace()
                .nth(1)?
                .parse()
                .ok()
        })
        .unwrap_or(0)
}

impl Helper {
    fn start() -> Result<Running, String> {
        let program = std::env::current_exe().map_err(|error| error.to_string())?;
        let mut child = Command::new(program)
            .arg(HELPER_ARG)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("starting the reading helper: {error}"))?;
        let stdin = child.stdin.take().ok_or("the helper has no input")?;
        let stdout = child.stdout.take().ok_or("the helper has no output")?;
        let (send, lines) = channel();
        std::thread::Builder::new()
            .name("kog-discover-helper-reader".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if send.send(line).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Running { child, stdin, lines, jobs: 0 })
    }

    /// Send one job and wait up to `timeout` for its answer. A helper that
    /// misses it, dies, or has done its share is replaced.
    fn call(&mut self, request: &Request, timeout: Duration) -> Result<Response, String> {
        if self.running.is_none() {
            self.running = Some(Self::start()?);
        }
        let running = self.running.as_mut().expect("started above");
        let text = serde_json::to_string(request).map_err(|error| error.to_string())?;
        let answer = writeln!(running.stdin, "{text}")
            .and_then(|()| running.stdin.flush())
            .map_err(|error| error.to_string())
            .and_then(|()| running.lines.recv_timeout(timeout).map_err(|_| "the helper took too long".to_owned()));
        running.jobs += 1;
        let worn = running.jobs >= JOBS_PER_HELPER || resident_kb(running.child.id()) > MAX_HELPER_KB;
        if answer.is_err() || worn {
            self.running = None;
        }
        let response: Response = serde_json::from_str(&answer?).map_err(|error| error.to_string())?;
        match response.error {
            Some(error) if response.songs.is_empty() && response.features.is_none() && response.clip.is_none() => {
                Err(error)
            }
            _ => Ok(response),
        }
    }

    /// Every song in one file or archive (none for a file that plays
    /// nothing or could not be read in time).
    pub fn read(&mut self, unit: &Path, added_at: i64, timeout: Duration) -> Vec<Song> {
        self.call(&Request::Read { unit: unit.to_path_buf(), added_at }, timeout)
            .map(|response| response.songs)
            .unwrap_or_else(|error| {
                eprintln!("kog: reading {} for the library index: {error}", unit.display());
                Vec::new()
            })
    }

    pub fn measure(&mut self, entry: &StoredEntry, timeout: Duration) -> Option<Vec<f32>> {
        self.call(&Request::Measure { entry: Locator::from(entry) }, timeout).ok()?.features
    }

    pub fn clip(&mut self, entry: &StoredEntry, timeout: Duration) -> Option<Vec<f32>> {
        let encoded = self.call(&Request::Clip { entry: Locator::from(entry) }, timeout).ok()?.clip?;
        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).ok()?;
        Some(
            bytes
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect(),
        )
    }
}
