//! Safe process wrapper for libupse PSF and Play! PSF2 playback.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Duration;

const HELPER_MAGIC: [u8; 8] = *b"KOGPSF1\0";
const HELPER_PROTOCOL_VERSION: u32 = 1;
const HELPER_METADATA_FIELDS: usize = 5;
const HELPER_MAX_METADATA_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PsfMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
}

pub struct Psf {
    path: PathBuf,
    process: Option<PsfProcess>,
    format_version: u32,
    sample_rate: u32,
    channels: u16,
    main_frames: u64,
    total_frames: u64,
    rendered_frames: u64,
    default_length_milliseconds: u32,
    default_fade_milliseconds: u32,
    metadata: PsfMetadata,
    native_bytes: Vec<u8>,
}

struct PsfProcess {
    child: Child,
    stdout: ChildStdout,
}

struct HelperHeader {
    format_version: u32,
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    main_frames: u64,
    metadata: PsfMetadata,
}

impl Psf {
    pub fn open(
        path: &Path,
        default_length: Duration,
        default_fade: Duration,
    ) -> Result<Self, String> {
        let default_length_milliseconds = duration_milliseconds(default_length, "length")?;
        let default_fade_milliseconds = duration_milliseconds(default_fade, "fade")?;
        let path = path.to_path_buf();
        let (process, header) = spawn_helper(
            &path,
            0,
            default_length_milliseconds,
            default_fade_milliseconds,
        )?;
        if let Err(error) = validate_header(&header, &path) {
            let mut process = process;
            stop_process(&mut process);
            return Err(error);
        }

        Ok(Self {
            path,
            process: Some(process),
            format_version: header.format_version,
            sample_rate: header.sample_rate,
            channels: header.channels,
            main_frames: header.main_frames,
            total_frames: header.total_frames,
            rendered_frames: 0,
            default_length_milliseconds,
            default_fade_milliseconds,
            metadata: header.metadata,
            native_bytes: Vec::new(),
        })
    }

    pub fn duration(&self) -> Duration {
        duration_from_frames(self.total_frames, self.sample_rate)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    pub fn metadata(&self) -> &PsfMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "PSF output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let remaining = self.total_frames.saturating_sub(self.rendered_frames);
        let requested = usize::try_from(remaining.min((output.len() / channels) as u64))
            .expect("requested PSF frames fit the output buffer");
        if requested == 0 {
            return Ok(0);
        }
        let byte_count = requested
            .checked_mul(channels)
            .and_then(|samples| samples.checked_mul(2))
            .ok_or_else(|| "PSF render request exceeds Kog's buffer limit".to_owned())?;
        self.native_bytes.resize(byte_count, 0);

        let read_result = self
            .process
            .as_mut()
            .ok_or_else(|| "PSF helper process is not running".to_owned())?
            .stdout
            .read_exact(&mut self.native_bytes);
        if let Err(error) = read_result {
            return Err(self.process_error(format!("reading PCM from the PSF helper: {error}")));
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
        let (process, header) = spawn_helper(
            &self.path,
            target,
            self.default_length_milliseconds,
            self.default_fade_milliseconds,
        )?;
        if header.format_version != self.format_version
            || header.sample_rate != self.sample_rate
            || header.channels != self.channels
            || header.main_frames != self.main_frames
            || header.total_frames != self.total_frames
        {
            let mut process = process;
            stop_process(&mut process);
            return Err("PSF helper reported different stream properties after seek".to_owned());
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

impl Drop for Psf {
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
                "invalid PSF helper protocol magic",
            ));
        }
        let protocol_version = read_u32_le(reader)?;
        if protocol_version != HELPER_PROTOCOL_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported PSF helper protocol {protocol_version}"),
            ));
        }
        let format_version = read_u32_le(reader)?;
        let sample_rate = read_u32_le(reader)?;
        let channels = u16::try_from(read_u32_le(reader)?)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid PSF channel count"))?;
        let total_frames = read_u64_le(reader)?;
        let main_frames = read_u64_le(reader)?;
        let mut lengths = [0_usize; HELPER_METADATA_FIELDS];
        let mut total_metadata = 0_usize;
        for length in &mut lengths {
            *length = usize::try_from(read_u32_le(reader)?).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "PSF metadata length overflow")
            })?;
            total_metadata = total_metadata.checked_add(*length).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "PSF metadata length overflow")
            })?;
        }
        if total_metadata > HELPER_MAX_METADATA_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PSF helper metadata exceeds Kog's limit",
            ));
        }
        let mut fields = Vec::with_capacity(HELPER_METADATA_FIELDS);
        for length in lengths {
            let mut bytes = vec![0_u8; length];
            reader.read_exact(&mut bytes)?;
            let text = String::from_utf8_lossy(&bytes).trim().to_owned();
            fields.push((!text.is_empty()).then_some(text));
        }

        Ok(Self {
            format_version,
            sample_rate,
            channels,
            total_frames,
            main_frames,
            metadata: PsfMetadata {
                title: fields[0].clone(),
                artist: fields[1].clone(),
                album: fields[2].clone(),
                genre: fields[3].clone(),
                date: fields[4].clone(),
            },
        })
    }
}

fn spawn_helper(
    path: &Path,
    start_frame: u64,
    default_length_milliseconds: u32,
    default_fade_milliseconds: u32,
) -> Result<(PsfProcess, HelperHeader), String> {
    let helper = helper_path(path)?;
    let mut child = Command::new(&helper)
        .arg(path)
        .arg(start_frame.to_string())
        .arg(default_length_milliseconds.to_string())
        .arg(default_fade_milliseconds.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("launching {}: {error}", helper.display()))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "PSF helper stdout was not captured".to_owned())?;
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
                    "opening {} with the PSF helper failed ({status}): {error}{detail}",
                    path.display()
                ),
                Err(wait_error) => format!(
                    "opening {} with the PSF helper failed: {error}; waiting failed: {wait_error}{detail}",
                    path.display()
                ),
            });
        }
    };
    Ok((PsfProcess { child, stdout }, header))
}

fn validate_header(header: &HelperHeader, path: &Path) -> Result<(), String> {
    let source_version = psf_format_version(path)?;
    if header.format_version != u32::from(source_version)
        || !matches!(header.format_version, 1 | 2)
        || header.sample_rate == 0
        || header.channels != 2
        || header.main_frames == 0
        || header.main_frames > header.total_frames
    {
        return Err(format!(
            "PSF helper reported invalid stream properties for {}",
            path.display()
        ));
    }
    Ok(())
}

fn helper_path(source: &Path) -> Result<PathBuf, String> {
    let version = psf_format_version(source)?;
    let (override_name, executable_name, build_helper) = match version {
        1 => (
            "KOG_PSF_HELPER",
            if cfg!(windows) {
                "kog-psf-helper.exe"
            } else {
                "kog-psf-helper"
            },
            PathBuf::from(env!("KOG_BUILD_PSF_HELPER")),
        ),
        2 => (
            "KOG_PSF2_HELPER",
            if cfg!(windows) {
                "kog-psf2-helper.exe"
            } else {
                "kog-psf2-helper"
            },
            PathBuf::from(env!("KOG_BUILD_PSF2_HELPER")),
        ),
        _ => {
            return Err(format!(
                "unsupported PSF format version {version} in {}",
                source.display()
            ));
        }
    };
    if let Some(path) = std::env::var_os(override_name) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{override_name} does not name a file: {}",
            path.display()
        ));
    }

    if let Ok(executable) = std::env::current_exe() {
        let sibling = executable.with_file_name(executable_name);
        if sibling.is_file() {
            return Ok(sibling);
        }
    }

    if build_helper.is_file() {
        return Ok(build_helper);
    }
    Err(format!(
        "PSF format {version} helper is not installed beside Kog and the build copy is missing: {}",
        build_helper.display()
    ))
}

fn psf_format_version(path: &Path) -> Result<u8, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("opening PSF header {}: {error}", path.display()))?;
    let mut header = [0_u8; 4];
    file.read_exact(&mut header)
        .map_err(|error| format!("reading PSF header {}: {error}", path.display()))?;
    if &header[..3] != b"PSF" {
        return Err(format!("invalid PSF signature in {}", path.display()));
    }
    Ok(header[3])
}

fn stop_process(process: &mut PsfProcess) {
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

fn duration_milliseconds(duration: Duration, label: &str) -> Result<u32, String> {
    u32::try_from(duration.as_millis())
        .map_err(|_| format!("default PSF {label} exceeds the native API limit"))
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "PSF duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond PSF duration fits u32"),
    )
}

#[cfg(test)]
pub fn test_psf_bytes(executable: Option<&[u8]>, tags: &str) -> Vec<u8> {
    let compressed = stored_zlib(executable.unwrap_or_default());
    let mut output = Vec::new();
    output.extend_from_slice(b"PSF\x01");
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(compressed.len()).unwrap().to_le_bytes());
    output.extend_from_slice(&crc32(&compressed).to_le_bytes());
    output.extend_from_slice(&compressed);
    output.extend_from_slice(b"[TAG]");
    output.extend_from_slice(tags.as_bytes());
    output
}

#[cfg(test)]
pub fn test_psf_executable() -> Vec<u8> {
    const LOAD_ADDRESS: u32 = 0x8001_0000;
    const T0: u32 = 8;
    const T1: u32 = 9;

    let mut program = Vec::new();
    push_instruction(&mut program, 0x3c00_0000 | (T0 << 16) | 0x1f80); // LUI t0,0x1f80
    push_instruction(&mut program, 0x3400_0000 | (T0 << 21) | (T0 << 16) | 0x1c00); // ORI t0,t0,0x1c00

    let write_spu = |program: &mut Vec<u8>, offset: u16, value: u16| {
        push_instruction(program, 0x3400_0000 | (T1 << 16) | u32::from(value));
        push_instruction(
            program,
            0xa400_0000 | (T0 << 21) | (T1 << 16) | u32::from(offset),
        );
    };

    write_spu(&mut program, 0x01aa, 0xc000); // SPU on + main output
    write_spu(&mut program, 0x01a6, 0x0200); // transfer address 0x1000
    for word in [
        0x0300, 0x7777, 0x9999, 0x7777, 0x9999, 0x7777, 0x9999, 0x7777,
    ] {
        write_spu(&mut program, 0x01a8, word);
    }
    write_spu(&mut program, 0x0180, 0x3fff); // main left
    write_spu(&mut program, 0x0182, 0x3fff); // main right
    write_spu(&mut program, 0x0000, 0x3fff); // voice 0 left
    write_spu(&mut program, 0x0002, 0x3fff); // voice 0 right
    write_spu(&mut program, 0x0004, 0x1000); // native pitch
    write_spu(&mut program, 0x0006, 0x0200); // sample start 0x1000
    write_spu(&mut program, 0x0008, 0x000f); // fast attack, full sustain
    write_spu(&mut program, 0x000a, 0x0000);
    write_spu(&mut program, 0x000e, 0x0200); // repeat address 0x1000
    write_spu(&mut program, 0x0188, 0x0001); // key on voice 0
    push_instruction(&mut program, 0x1000_ffff); // loop: BEQ zero,zero,loop
    push_instruction(&mut program, 0); // delay slot

    let mut executable = vec![0_u8; 2048];
    executable[0..8].copy_from_slice(b"PS-X EXE");
    executable[0x10..0x14].copy_from_slice(&LOAD_ADDRESS.to_le_bytes());
    executable[0x18..0x1c].copy_from_slice(&LOAD_ADDRESS.to_le_bytes());
    executable[0x1c..0x20].copy_from_slice(&u32::try_from(program.len()).unwrap().to_le_bytes());
    executable[0x30..0x34].copy_from_slice(&0x801f_ff00_u32.to_le_bytes());
    executable[113..126].copy_from_slice(b"North America");
    executable.extend_from_slice(&program);
    executable
}

#[cfg(test)]
pub fn test_psf_out_of_bounds_executable() -> Vec<u8> {
    let mut executable = vec![0_u8; 2048 + 8];
    executable[0..8].copy_from_slice(b"PS-X EXE");
    executable[0x10..0x14].copy_from_slice(&0x801f_fffc_u32.to_le_bytes());
    executable[0x18..0x1c].copy_from_slice(&0x801f_fffc_u32.to_le_bytes());
    executable[0x1c..0x20].copy_from_slice(&8_u32.to_le_bytes());
    executable
}

#[cfg(test)]
pub fn test_psf2_bytes(files: &[(&str, &[u8])], tags: &str) -> Vec<u8> {
    let reserved = test_psf2_filesystem(files);
    let mut output = Vec::new();
    output.extend_from_slice(b"PSF\x02");
    output.extend_from_slice(&u32::try_from(reserved.len()).unwrap().to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&reserved);
    output.extend_from_slice(b"[TAG]");
    output.extend_from_slice(tags.as_bytes());
    output
}

#[cfg(test)]
pub fn test_psf2_irx() -> Vec<u8> {
    const T0: u32 = 8;
    const T1: u32 = 9;
    const ELF_HEADER_BYTES: usize = 52;
    const PROGRAM_HEADER_BYTES: usize = 32;
    const CODE_OFFSET: usize = 0x100;
    const IOPMOD_BYTES: usize = 282;

    let mut program = Vec::new();
    push_instruction(&mut program, 0x3c00_0000 | (T0 << 16) | 0x1f90); // LUI t0,0x1f90
    let write_spu2 = |program: &mut Vec<u8>, offset: u16, value: u16| {
        push_instruction(program, 0x3400_0000 | (T1 << 16) | u32::from(value));
        push_instruction(
            program,
            0xa400_0000 | (T0 << 21) | (T1 << 16) | u32::from(offset),
        );
    };

    write_spu2(&mut program, 0x019a, 0x8000); // enable SPU2 core 0
    write_spu2(&mut program, 0x01a8, 0x0000); // transfer address high
    write_spu2(&mut program, 0x01aa, 0x0800); // transfer address 0x1000
    for word in [
        0x0300, 0x7777, 0x9999, 0x7777, 0x9999, 0x7777, 0x9999, 0x7777,
    ] {
        write_spu2(&mut program, 0x01ac, word);
    }
    write_spu2(&mut program, 0x0000, 0x3fff); // voice 0 left
    write_spu2(&mut program, 0x0002, 0x3fff); // voice 0 right
    write_spu2(&mut program, 0x0004, 0x1000); // native pitch
    write_spu2(&mut program, 0x0006, 0x000f); // fast attack, full sustain
    write_spu2(&mut program, 0x0008, 0x0000);
    write_spu2(&mut program, 0x01c0, 0x0000); // sample address high
    write_spu2(&mut program, 0x01c2, 0x0800); // sample address 0x1000
    write_spu2(&mut program, 0x01c4, 0x0000); // repeat address high
    write_spu2(&mut program, 0x01c6, 0x0800); // repeat address 0x1000
    write_spu2(&mut program, 0x01a0, 0x0001); // key on voice 0
    push_instruction(&mut program, 0x1000_ffff); // loop
    push_instruction(&mut program, 0); // branch delay slot

    let iopmod_offset = align4(CODE_OFFSET + program.len());
    let section_offset = align4(iopmod_offset + IOPMOD_BYTES);
    let mut elf = vec![0_u8; section_offset + 80];
    elf[0..4].copy_from_slice(b"\x7fELF");
    elf[4] = 1; // ELF32
    elf[5] = 1; // little endian
    elf[6] = 1; // current version
    write_u16_at(&mut elf, 16, 0xff80); // relocatable IOP executable
    write_u16_at(&mut elf, 18, 8); // MIPS
    write_u32_at(&mut elf, 20, 1);
    write_u32_at(&mut elf, 24, 0); // entry is relative to allocated base
    write_u32_at(&mut elf, 28, u32::try_from(ELF_HEADER_BYTES).unwrap());
    write_u32_at(&mut elf, 32, u32::try_from(section_offset).unwrap());
    write_u16_at(&mut elf, 40, u16::try_from(ELF_HEADER_BYTES).unwrap());
    write_u16_at(&mut elf, 42, u16::try_from(PROGRAM_HEADER_BYTES).unwrap());
    write_u16_at(&mut elf, 44, 1);
    write_u16_at(&mut elf, 46, 40);
    write_u16_at(&mut elf, 48, 2);

    write_u32_at(&mut elf, ELF_HEADER_BYTES, 1); // PT_LOAD
    write_u32_at(
        &mut elf,
        ELF_HEADER_BYTES + 4,
        u32::try_from(CODE_OFFSET).unwrap(),
    );
    write_u32_at(
        &mut elf,
        ELF_HEADER_BYTES + 16,
        u32::try_from(program.len()).unwrap(),
    );
    write_u32_at(
        &mut elf,
        ELF_HEADER_BYTES + 20,
        u32::try_from(program.len()).unwrap(),
    );
    write_u32_at(&mut elf, ELF_HEADER_BYTES + 24, 7); // read/write/execute
    write_u32_at(&mut elf, ELF_HEADER_BYTES + 28, 16);
    elf[CODE_OFFSET..CODE_OFFSET + program.len()].copy_from_slice(&program);

    write_u32_at(
        &mut elf,
        iopmod_offset + 12,
        u32::try_from(program.len()).unwrap(),
    );
    write_u16_at(&mut elf, iopmod_offset + 24, 0x0100);
    elf[iopmod_offset + 26..iopmod_offset + 34].copy_from_slice(b"kogpsf2\0");

    let iopmod_section = section_offset + 40;
    write_u32_at(&mut elf, iopmod_section + 4, 0x7000_0080);
    write_u32_at(
        &mut elf,
        iopmod_section + 16,
        u32::try_from(iopmod_offset).unwrap(),
    );
    write_u32_at(
        &mut elf,
        iopmod_section + 20,
        u32::try_from(IOPMOD_BYTES).unwrap(),
    );
    write_u32_at(&mut elf, iopmod_section + 32, 4);
    elf
}

#[cfg(test)]
fn test_psf2_filesystem(files: &[(&str, &[u8])]) -> Vec<u8> {
    let table_bytes = 4 + files.len() * 48;
    let mut output = vec![0_u8; table_bytes];
    write_u32_at(&mut output, 0, u32::try_from(files.len()).unwrap());
    for (index, (name, data)) in files.iter().enumerate() {
        assert!(!data.is_empty());
        assert!(name.len() < 36 && !name.contains('/') && !name.contains('\\'));
        let compressed = stored_zlib(data);
        let data_offset = output.len();
        output.extend_from_slice(&u32::try_from(compressed.len()).unwrap().to_le_bytes());
        output.extend_from_slice(&compressed);

        let entry = 4 + index * 48;
        output[entry..entry + name.len()].copy_from_slice(name.as_bytes());
        write_u32_at(&mut output, entry + 36, u32::try_from(data_offset).unwrap());
        write_u32_at(&mut output, entry + 40, u32::try_from(data.len()).unwrap());
        write_u32_at(&mut output, entry + 44, u32::try_from(data.len()).unwrap());
    }
    output
}

#[cfg(test)]
fn align4(value: usize) -> usize {
    (value + 3) & !3
}

#[cfg(test)]
fn write_u16_at(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
fn write_u32_at(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
fn push_instruction(program: &mut Vec<u8>, instruction: u32) {
    program.extend_from_slice(&instruction.to_le_bytes());
}

#[cfg(test)]
fn stored_zlib(data: &[u8]) -> Vec<u8> {
    assert!(data.len() <= usize::from(u16::MAX));
    let length = u16::try_from(data.len()).unwrap();
    let mut output = vec![0x78, 0x01, 0x01];
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(&(!length).to_le_bytes());
    output.extend_from_slice(data);
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

#[cfg(test)]
fn adler32(bytes: &[u8]) -> u32 {
    let mut a = 1_u32;
    let mut b = 0_u32;
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

#[cfg(test)]
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
