//! Process wrapper for Cog's portable GME SFM renderer.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Duration;

const HELPER_MAGIC: [u8; 8] = *b"KOGSFM1\0";
const HELPER_PROTOCOL_VERSION: u32 = 1;
const HELPER_MAX_STRING_BYTES: usize = 64 * 1024;

pub struct Sfm {
    path: PathBuf,
    process: Option<SfmProcess>,
    sample_rate: u32,
    channels: u16,
    main_frames: u64,
    total_frames: u64,
    rendered_frames: u64,
    system: String,
    title: String,
    game: String,
    author: String,
    copyright: String,
    date: String,
    native_bytes: Vec<u8>,
}

struct SfmProcess {
    child: Child,
    stdout: ChildStdout,
}

#[derive(Debug, PartialEq, Eq)]
struct HelperHeader {
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    main_frames: u64,
    system: String,
    title: String,
    game: String,
    author: String,
    copyright: String,
    date: String,
}

impl Sfm {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path.to_path_buf();
        let (process, header) = spawn_helper(&path, 0)?;
        if let Err(error) = validate_header(&header, &path) {
            let mut process = process;
            stop_process(&mut process);
            return Err(error);
        }
        Ok(Self {
            path,
            process: Some(process),
            sample_rate: header.sample_rate,
            channels: header.channels,
            main_frames: header.main_frames,
            total_frames: header.total_frames,
            rendered_frames: 0,
            system: header.system,
            title: header.title,
            game: header.game,
            author: header.author,
            copyright: header.copyright,
            date: header.date,
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

    pub fn system(&self) -> &str {
        &self.system
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn game(&self) -> &str {
        &self.game
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn date(&self) -> &str {
        &self.date
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "SFM output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let remaining = self.total_frames.saturating_sub(self.rendered_frames);
        let requested = usize::try_from(remaining.min((output.len() / channels) as u64))
            .expect("requested SFM frames fit the output buffer");
        if requested == 0 {
            return Ok(0);
        }
        let byte_count = requested
            .checked_mul(channels)
            .and_then(|samples| samples.checked_mul(2))
            .ok_or_else(|| "SFM render request exceeds Kog's buffer limit".to_owned())?;
        self.native_bytes.resize(byte_count, 0);
        if let Err(error) = self
            .process
            .as_mut()
            .ok_or_else(|| "SFM helper process is not running".to_owned())?
            .stdout
            .read_exact(&mut self.native_bytes)
        {
            return Err(self.process_error(format!("reading PCM from the SFM helper: {error}")));
        }
        for (sample, bytes) in output[..requested * channels]
            .iter_mut()
            .zip(self.native_bytes.chunks_exact(2))
        {
            *sample = f32::from(i16::from_le_bytes([bytes[0], bytes[1]])) * (1.0 / 32_768.0);
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
        let (process, header) = spawn_helper(&self.path, target)?;
        if header.sample_rate != self.sample_rate
            || header.channels != self.channels
            || header.main_frames != self.main_frames
            || header.total_frames != self.total_frames
            || header.system != self.system
            || header.title != self.title
            || header.game != self.game
            || header.author != self.author
            || header.copyright != self.copyright
            || header.date != self.date
        {
            let mut process = process;
            stop_process(&mut process);
            return Err("SFM helper reported different stream properties after seek".to_owned());
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

impl Drop for Sfm {
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
                "invalid SFM helper protocol magic",
            ));
        }
        let version = read_u32_le(reader)?;
        if version != HELPER_PROTOCOL_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported SFM helper protocol {version}"),
            ));
        }
        Ok(Self {
            sample_rate: read_u32_le(reader)?,
            channels: u16::try_from(read_u32_le(reader)?).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid SFM channel count")
            })?,
            total_frames: read_u64_le(reader)?,
            main_frames: read_u64_le(reader)?,
            system: read_string(reader)?,
            title: read_string(reader)?,
            game: read_string(reader)?,
            author: read_string(reader)?,
            copyright: read_string(reader)?,
            date: read_string(reader)?,
        })
    }
}

fn validate_header(header: &HelperHeader, path: &Path) -> Result<(), String> {
    if header.sample_rate != 32_000
        || header.channels != 2
        || header.main_frames == 0
        || header.main_frames > header.total_frames
    {
        return Err(format!(
            "SFM helper reported invalid stream properties for {}",
            path.display()
        ));
    }
    Ok(())
}

fn spawn_helper(path: &Path, start_frame: u64) -> Result<(SfmProcess, HelperHeader), String> {
    let helper = helper_path()?;
    let mut child = Command::new(&helper)
        .arg(path)
        .arg(start_frame.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("launching {}: {error}", helper.display()))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "SFM helper stdout was not captured".to_owned())?;
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
                    "opening {} with the SFM helper failed ({status}): {error}{detail}",
                    path.display()
                ),
                Err(wait_error) => format!(
                    "opening {} with the SFM helper failed: {error}; waiting failed: {wait_error}{detail}",
                    path.display()
                ),
            });
        }
    };
    Ok((SfmProcess { child, stdout }, header))
}

fn helper_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("KOG_SFM_HELPER") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "KOG_SFM_HELPER does not name a file: {}",
            path.display()
        ));
    }
    let executable_name = if cfg!(windows) {
        "kog-sfm-helper.exe"
    } else {
        "kog-sfm-helper"
    };
    if let Ok(executable) = std::env::current_exe() {
        let sibling = executable.with_file_name(executable_name);
        if sibling.is_file() {
            return Ok(sibling);
        }
    }
    let build_helper = PathBuf::from(env!("KOG_BUILD_SFM_HELPER"));
    if build_helper.is_file() {
        return Ok(build_helper);
    }
    Err(format!(
        "SFM helper is not installed beside Kog and the build copy is missing: {}",
        build_helper.display()
    ))
}

fn stop_process(process: &mut SfmProcess) {
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

fn read_string(reader: &mut impl Read) -> io::Result<String> {
    let length = usize::try_from(read_u32_le(reader)?)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "SFM string length overflow"))?;
    if length > HELPER_MAX_STRING_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SFM helper metadata string exceeds Kog's limit",
        ));
    }
    let mut bytes = vec![0_u8; length];
    reader.read_exact(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_owned())
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "SFM duration exceeds Kog's limit".to_owned())
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
pub fn test_sfm_bytes() -> Vec<u8> {
    // Original SPC700 program shared with Kog's synthetic SNSF gate. It writes
    // a small generated BRR waveform and loops voice zero. No Nintendo ROM or
    // game data is embedded.
    const SPC_PROGRAM: &[u8] = &[
        0xe8, 0x10, 0xc5, 0x00, 0x03, 0xe8, 0x03, 0xc5, 0x01, 0x03, 0xe8, 0x10, 0xc5, 0x02, 0x03,
        0xe8, 0x03, 0xc5, 0x03, 0x03, 0xe8, 0x93, 0xc5, 0x10, 0x03, 0xe8, 0x77, 0xc5, 0x11, 0x03,
        0xc5, 0x12, 0x03, 0xe8, 0x99, 0xc5, 0x13, 0x03, 0xc5, 0x14, 0x03, 0xe8, 0x77, 0xc5, 0x15,
        0x03, 0xc5, 0x16, 0x03, 0xe8, 0x99, 0xc5, 0x17, 0x03, 0xc5, 0x18, 0x03, 0xe8, 0x6c, 0xc4,
        0xf2, 0xe8, 0x00, 0xc4, 0xf3, 0xe8, 0x5d, 0xc4, 0xf2, 0xe8, 0x03, 0xc4, 0xf3, 0xe8, 0x0c,
        0xc4, 0xf2, 0xe8, 0x7f, 0xc4, 0xf3, 0xe8, 0x1c, 0xc4, 0xf2, 0xe8, 0x7f, 0xc4, 0xf3, 0xe8,
        0x00, 0xc4, 0xf2, 0xe8, 0x60, 0xc4, 0xf3, 0xe8, 0x01, 0xc4, 0xf2, 0xe8, 0x60, 0xc4, 0xf3,
        0xe8, 0x02, 0xc4, 0xf2, 0xe8, 0x00, 0xc4, 0xf3, 0xe8, 0x03, 0xc4, 0xf2, 0xe8, 0x10, 0xc4,
        0xf3, 0xe8, 0x04, 0xc4, 0xf2, 0xe8, 0x00, 0xc4, 0xf3, 0xe8, 0x05, 0xc4, 0xf2, 0xe8, 0x8f,
        0xc4, 0xf3, 0xe8, 0x06, 0xc4, 0xf2, 0xe8, 0xe0, 0xc4, 0xf3, 0xe8, 0x4c, 0xc4, 0xf2, 0xe8,
        0x01, 0xc4, 0xf3, 0x2f, 0xfe,
    ];
    const METADATA: &str = "information\n  title: Synthetic SFM\n  game: Kog test suite\n  author: Kog tests\n  copyright: Original generated fixture\n  date: 2026-08-31\ntiming\n  length: 500\n  fade: 100\n  loopstart: 0\nsmp\n  iplrom: 0\n  regs\n    pc: 512\n    a: 0\n    x: 0\n    y: 0\n    s: 239\n    psw: 2\ndsp\n  voice\n    vbit: 1\n    vidx: 0\n  voice\n    vbit: 2\n    vidx: 16\n  voice\n    vbit: 4\n    vidx: 32\n  voice\n    vbit: 8\n    vidx: 48\n  voice\n    vbit: 16\n    vidx: 64\n  voice\n    vbit: 32\n    vidx: 80\n  voice\n    vbit: 64\n    vidx: 96\n  voice\n    vbit: 128\n    vidx: 112\n";

    let mut ram = vec![0_u8; 65_536];
    ram[0x0200..0x0200 + SPC_PROGRAM.len()].copy_from_slice(SPC_PROGRAM);
    let metadata = METADATA.as_bytes();
    let mut output = Vec::with_capacity(8 + metadata.len() + ram.len() + 128 + 1);
    output.extend_from_slice(b"SFM1");
    output.extend_from_slice(&u32::try_from(metadata.len()).unwrap().to_le_bytes());
    output.extend_from_slice(metadata);
    output.extend_from_slice(&ram);
    output.extend_from_slice(&[0_u8; 128]);
    output.push(0);
    output
}
