//! Process wrapper for the pinned syntrax-c JXS renderer.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Duration;

const HELPER_MAGIC: [u8; 8] = *b"KOGJXS1\0";
const HELPER_PROTOCOL_VERSION: u32 = 1;
const HELPER_MAX_TITLE_BYTES: usize = 64 * 1024;

pub struct Syntrax {
    path: PathBuf,
    selected_subsong: u32,
    process: Option<SyntraxProcess>,
    sample_rate: u32,
    channels: u16,
    main_frames: u64,
    total_frames: u64,
    rendered_frames: u64,
    subsong_count: u32,
    title: String,
    native_bytes: Vec<u8>,
}

struct SyntraxProcess {
    child: Child,
    stdout: ChildStdout,
}

#[derive(Debug)]
struct HelperHeader {
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    main_frames: u64,
    subsong_count: u32,
    selected_subsong: u32,
    title: String,
}

impl Syntrax {
    pub fn open(path: &Path, subsong: Option<u32>) -> Result<Self, String> {
        let selected_subsong = subsong.unwrap_or(0);
        let path = path.to_path_buf();
        let (process, header) = spawn_helper(&path, selected_subsong, 0)?;
        if let Err(error) = validate_header(&header, selected_subsong, &path) {
            let mut process = process;
            stop_process(&mut process);
            return Err(error);
        }
        Ok(Self {
            path,
            selected_subsong,
            process: Some(process),
            sample_rate: header.sample_rate,
            channels: header.channels,
            main_frames: header.main_frames,
            total_frames: header.total_frames,
            rendered_frames: 0,
            subsong_count: header.subsong_count,
            title: header.title,
            native_bytes: Vec::new(),
        })
    }

    pub fn duration(&self) -> Duration {
        duration_from_frames(self.total_frames, self.sample_rate)
    }

    #[cfg(test)]
    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn selected_subsong(&self) -> u32 {
        self.selected_subsong
    }

    pub fn subsong_count(&self) -> u32 {
        self.subsong_count
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "Syntrax output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let remaining = self.total_frames.saturating_sub(self.rendered_frames);
        let requested = usize::try_from(remaining.min((output.len() / channels) as u64))
            .expect("requested Syntrax frames fit the output buffer");
        if requested == 0 {
            return Ok(0);
        }
        let byte_count = requested
            .checked_mul(channels)
            .and_then(|samples| samples.checked_mul(2))
            .ok_or_else(|| "Syntrax render request exceeds Kog's buffer limit".to_owned())?;
        self.native_bytes.resize(byte_count, 0);
        if let Err(error) = self
            .process
            .as_mut()
            .ok_or_else(|| "Syntrax helper process is not running".to_owned())?
            .stdout
            .read_exact(&mut self.native_bytes)
        {
            return Err(self.process_error(format!("reading PCM from the Syntrax helper: {error}")));
        }

        let fade_frames = self.total_frames.saturating_sub(self.main_frames);
        for frame in 0..requested {
            let absolute_frame = self.rendered_frames + frame as u64;
            let gain = if fade_frames != 0 && absolute_frame >= self.main_frames {
                (self.total_frames - absolute_frame) as f32 / fade_frames as f32
            } else {
                1.0
            };
            for channel in 0..channels {
                let sample = frame * channels + channel;
                let byte = sample * 2;
                output[sample] =
                    i16::from_le_bytes([self.native_bytes[byte], self.native_bytes[byte + 1]])
                        as f32
                        * (gain / 32768.0);
            }
        }
        self.rendered_frames += requested as u64;
        Ok(requested)
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = if position >= self.duration() {
            self.total_frames
        } else {
            frames_from_duration(position, self.sample_rate)?
        };
        let (process, header) = spawn_helper(&self.path, self.selected_subsong, target)?;
        if header.sample_rate != self.sample_rate
            || header.channels != self.channels
            || header.main_frames != self.main_frames
            || header.total_frames != self.total_frames
            || header.subsong_count != self.subsong_count
            || header.selected_subsong != self.selected_subsong
            || header.title != self.title
        {
            let mut process = process;
            stop_process(&mut process);
            return Err(
                "Syntrax helper reported different stream properties after seek".to_owned(),
            );
        }
        if let Some(mut old_process) = self.process.replace(process) {
            stop_process(&mut old_process);
        }
        self.rendered_frames = target;
        Ok(duration_from_frames(target, self.sample_rate))
    }

    fn process_error(&mut self, context: String) -> String {
        let Some(mut process) = self.process.take() else {
            return context;
        };
        drop(process.stdout);
        let status = process.child.wait();
        let stderr = read_stderr(&mut process.child);
        match (status, stderr.is_empty()) {
            (Ok(status), false) => format!("{context}; helper exited {status}: {stderr}"),
            (Ok(status), true) => format!("{context}; helper exited {status}"),
            (Err(error), false) => {
                format!("{context}; waiting for helper failed: {error}: {stderr}")
            }
            (Err(error), true) => format!("{context}; waiting for helper failed: {error}"),
        }
    }
}

impl Drop for Syntrax {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            stop_process(&mut process);
        }
    }
}

impl HelperHeader {
    fn read(reader: &mut impl Read) -> io::Result<Self> {
        let mut magic = [0_u8; 8];
        reader.read_exact(&mut magic)?;
        if magic != HELPER_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Syntrax helper protocol magic",
            ));
        }
        let version = read_u32_le(reader)?;
        if version != HELPER_PROTOCOL_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported Syntrax helper protocol {version}"),
            ));
        }
        let sample_rate = read_u32_le(reader)?;
        let channels = u16::try_from(read_u32_le(reader)?).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid Syntrax channel count")
        })?;
        let total_frames = read_u64_le(reader)?;
        let main_frames = read_u64_le(reader)?;
        let subsong_count = read_u32_le(reader)?;
        let selected_subsong = read_u32_le(reader)?;
        let title_length = usize::try_from(read_u32_le(reader)?).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Syntrax title length overflow")
        })?;
        if title_length > HELPER_MAX_TITLE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Syntrax helper title exceeds Kog's limit",
            ));
        }
        let mut title = vec![0_u8; title_length];
        reader.read_exact(&mut title)?;
        Ok(Self {
            sample_rate,
            channels,
            total_frames,
            main_frames,
            subsong_count,
            selected_subsong,
            title: String::from_utf8_lossy(&title).trim().to_owned(),
        })
    }
}

fn validate_header(header: &HelperHeader, subsong: u32, path: &Path) -> Result<(), String> {
    if header.sample_rate != 44_100
        || header.channels != 2
        || header.main_frames == 0
        || header.main_frames > header.total_frames
        || header.subsong_count == 0
        || header.selected_subsong != subsong
        || subsong >= header.subsong_count
    {
        return Err(format!(
            "Syntrax helper reported invalid stream properties for {}",
            path.display()
        ));
    }
    Ok(())
}

fn spawn_helper(
    path: &Path,
    subsong: u32,
    start_frame: u64,
) -> Result<(SyntraxProcess, HelperHeader), String> {
    let helper = helper_path()?;
    let mut child = Command::new(&helper)
        .arg(path)
        .arg(subsong.to_string())
        .arg(start_frame.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("launching {}: {error}", helper.display()))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Syntrax helper stdout was not captured".to_owned())?;
    let header = match HelperHeader::read(&mut stdout) {
        Ok(header) => header,
        Err(error) => {
            drop(stdout);
            let _ = child.kill();
            let status = child.wait();
            let stderr = read_stderr(&mut child);
            let detail = if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            };
            return Err(match status {
                Ok(status) => format!(
                    "opening {} with the Syntrax helper failed ({status}): {error}{detail}",
                    path.display()
                ),
                Err(wait_error) => format!(
                    "opening {} with the Syntrax helper failed: {error}; waiting failed: {wait_error}{detail}",
                    path.display()
                ),
            });
        }
    };
    Ok((SyntraxProcess { child, stdout }, header))
}

fn helper_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("KOG_SYNTRAX_HELPER") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "KOG_SYNTRAX_HELPER does not name a file: {}",
            path.display()
        ));
    }
    let executable_name = if cfg!(windows) {
        "kog-syntrax-helper.exe"
    } else {
        "kog-syntrax-helper"
    };
    if let Ok(executable) = std::env::current_exe() {
        let sibling = executable.with_file_name(executable_name);
        if sibling.is_file() {
            return Ok(sibling);
        }
    }
    let build_helper = PathBuf::from(env!("KOG_BUILD_SYNTRAX_HELPER"));
    if build_helper.is_file() {
        return Ok(build_helper);
    }
    Err(format!(
        "Syntrax helper is not installed beside Kog and the build copy is missing: {}",
        build_helper.display()
    ))
}

fn stop_process(process: &mut SyntraxProcess) {
    let _ = process.child.kill();
    let _ = process.child.wait();
}

fn read_stderr(child: &mut Child) -> String {
    let mut bytes = Vec::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_end(&mut bytes);
    }
    String::from_utf8_lossy(&bytes).trim().to_owned()
}

fn read_u32_le(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64_le(reader: &mut impl Read) -> io::Result<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "Syntrax duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    Duration::new(
        seconds,
        (remainder * 1_000_000_000 / u64::from(sample_rate)) as u32,
    )
}

#[cfg(test)]
pub fn test_jxs_bytes() -> Vec<u8> {
    const HEADER_BYTES: usize = 52;
    const SUBSONG_BYTES: usize = 16_564;
    const ROW_BYTES: usize = 5;
    const ROWS: usize = 64;
    const INSTRUMENT_BYTES: usize = 520;
    const WAVE_SAMPLES: usize = 16 * 256;

    fn put_i16(bytes: &mut [u8], offset: usize, value: i16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    fn put_i32(bytes: &mut [u8], offset: usize, value: i32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    let mut result = vec![0_u8; HEADER_BYTES];
    put_i16(&mut result, 0, 3457);
    put_i32(&mut result, 4, 1);
    put_i32(&mut result, 8, 2);
    put_i32(&mut result, 12, 1);

    for title in ["Synthetic JXS A", "Synthetic JXS B"] {
        let mut subsong = vec![0_u8; SUBSONG_BYTES];
        put_i32(&mut subsong, 80, 120);
        put_i32(&mut subsong, 88, 0);
        put_i32(&mut subsong, 92, 1);
        put_i32(&mut subsong, 96, 0);
        put_i32(&mut subsong, 100, 4);
        put_i32(&mut subsong, 104, 0);
        put_i32(&mut subsong, 108, 1);
        subsong[114..114 + title.len()].copy_from_slice(title.as_bytes());
        put_i16(&mut subsong, 146, 1);
        put_i16(&mut subsong, 148, 1);
        put_i16(&mut subsong, 166, 100);
        for channel in 0..16 {
            let order = 180 + channel * 256 * 4;
            put_i16(&mut subsong, order, 0);
            put_i16(&mut subsong, order + 2, 64);
        }
        result.extend_from_slice(&subsong);
    }

    let mut pattern = vec![0_u8; ROW_BYTES * ROWS];
    pattern[ROW_BYTES] = 45;
    pattern[ROW_BYTES + 2] = 1;
    result.extend_from_slice(&pattern);
    result.extend_from_slice(&1_i32.to_le_bytes());
    result.push(0);

    let mut instrument = vec![0_u8; INSTRUMENT_BYTES];
    put_i16(&mut instrument, 0, 3457);
    instrument[2..16].copy_from_slice(b"Synthetic tone");
    put_i16(&mut instrument, 34, 0);
    put_i16(&mut instrument, 36, 256);
    put_i16(&mut instrument, 38, 256);
    put_i16(&mut instrument, 46, 0);
    put_i16(&mut instrument, 56, 0);
    instrument[58] = 1;
    result.extend_from_slice(&instrument);
    for sample in 0..WAVE_SAMPLES {
        let value = if sample < 128 {
            12_000_i16
        } else if sample < 256 {
            -12_000_i16
        } else {
            0
        };
        result.extend_from_slice(&value.to_le_bytes());
    }
    result.extend_from_slice(&[0_u8; 16 * 16]);
    result
}

#[cfg(test)]
fn test_looping_jxs_bytes() -> Vec<u8> {
    const HEADER_BYTES: usize = 52;
    const SUBSONG_BYTES: usize = 16_564;
    const SONG_LOOP_OFFSET: usize = 112;
    let mut bytes = test_jxs_bytes();
    for subsong in 0..2 {
        let offset = HEADER_BYTES + subsong * SUBSONG_BYTES + SONG_LOOP_OFFSET;
        bytes[offset..offset + 2].copy_from_slice(&1_i16.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(test_name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "kog-syntrax-core-{}-{test_name}.jxs",
            std::process::id()
        ));
        std::fs::write(&path, test_jxs_bytes()).expect("write generated JXS fixture");
        path
    }

    #[test]
    fn generated_jxs_renders_subsongs_non_silent_pcm_and_seeks() {
        let path = fixture_path("render");
        let mut decoder = Syntrax::open(&path, Some(1)).expect("open generated JXS");
        assert_eq!(decoder.subsong_count(), 2);
        assert_eq!(decoder.selected_subsong(), 1);
        assert_eq!(decoder.title(), "Synthetic JXS B");
        assert_eq!(decoder.sample_rate(), 44_100);
        assert_eq!(decoder.channels(), 2);
        assert!(decoder.duration() > Duration::from_millis(100));
        let mut pcm = vec![0.0_f32; 4_410 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render JXS"), 4_410);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));
        assert!(decoder.seek(Duration::from_millis(50)).is_ok());
        pcm.fill(0.0);
        assert_eq!(decoder.render(&mut pcm).expect("render sought JXS"), 4_410);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn malformed_counts_and_missing_subsongs_are_rejected() {
        let path = fixture_path("malformed");
        let mut malformed = test_jxs_bytes();
        malformed[4..8].copy_from_slice(&(-1_i32).to_le_bytes());
        std::fs::write(&path, malformed).expect("write malformed JXS");
        assert!(Syntrax::open(&path, Some(0)).is_err());
        std::fs::write(&path, test_jxs_bytes()).expect("restore generated JXS");
        assert!(Syntrax::open(&path, Some(2)).is_err());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn looping_jxs_gets_cog_fade_and_exact_end_of_stream() {
        let path = fixture_path("looping");
        std::fs::write(&path, test_looping_jxs_bytes()).expect("write looping JXS fixture");
        let mut decoder = Syntrax::open(&path, Some(0)).expect("open looping JXS");
        assert!(decoder.duration() > Duration::from_secs(8));
        let seek = decoder.duration() - Duration::from_millis(10);
        decoder.seek(seek).expect("seek into Syntrax fade");
        let mut pcm = vec![0.0_f32; 1_000 * 2];
        let frames = decoder.render(&mut pcm).expect("render Syntrax fade tail");
        assert!((400..=450).contains(&frames));
        assert!(
            pcm[..frames * 2]
                .iter()
                .any(|sample| sample.abs() > 0.000_001)
        );
        assert!(
            pcm[(frames.saturating_sub(16)) * 2..frames * 2]
                .iter()
                .all(|sample| sample.abs() < 0.01)
        );
        assert_eq!(decoder.render(&mut pcm).expect("read Syntrax EOS"), 0);
        std::fs::remove_file(path).ok();
    }
}
