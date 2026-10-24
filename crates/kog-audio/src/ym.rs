//! YM register dumps (Leonard/Arnaud Carré's YM2!-YM6! formats, usually
//! LHA-compressed): the Atari ST's YM2149 or the CPC/Spectrum AY-3-8910
//! registers captured once per frame, played back through ayumi.
//!
//! The YM5!/YM6! special effects (SID voice, digidrums, Sync Buzzer) are not
//! reproduced; the plain tone, noise and envelope parts of a tune are.

use std::ffi::c_void;
use std::io::Read;
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

unsafe extern "C" {
    fn kog_ayumi_new(is_ym: i32, clock_rate: f64, sample_rate: i32, stereo: i32) -> *mut c_void;
    fn kog_ayumi_free(chip: *mut c_void);
    fn kog_ayumi_set_registers(chip: *mut c_void, registers: *const u8);
    fn kog_ayumi_render(chip: *mut c_void, output: *mut f32, frames: usize);
}

/// A parsed YM tune: one register set per frame.
pub struct YmSong {
    pub frames: Vec<[u8; 16]>,
    pub frame_rate: u32,
    pub clock: u32,
    pub loop_frame: Option<u32>,
    pub title: String,
    pub author: String,
    pub comment: String,
}

const MAX_YM_BYTES: usize = 64 * 1024 * 1024;

impl YmSong {
    pub fn open(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
        let bytes = if raw.get(2..5) == Some(b"-lh") {
            let mut reader = delharc::LhaDecodeReader::new(std::io::Cursor::new(raw))
                .map_err(|error| format!("{}: not a readable LHA archive: {error}", path.display()))?;
            let mut bytes = Vec::new();
            reader
                .by_ref()
                .take(MAX_YM_BYTES as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("{}: unpacking YM data: {error}", path.display()))?;
            bytes
        } else {
            raw
        };
        Self::parse(&bytes).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let u16_at = |at: usize| bytes.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
        let u32_at = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
        match bytes.get(0..4) {
            Some(b"YM2!" | b"YM3!" | b"YM3b") => {
                let looped = &bytes[0..4] == b"YM3b";
                let body = &bytes[4..bytes.len() - if looped { 4 } else { 0 }];
                let count = body.len() / 14;
                if count == 0 {
                    return Err("YM file has no frames".to_owned());
                }
                let frames = (0..count)
                    .map(|frame| {
                        let mut registers = [0_u8; 16];
                        for (register, value) in registers.iter_mut().take(14).enumerate() {
                            *value = body[register * count + frame];
                        }
                        registers
                    })
                    .collect();
                let loop_frame = looped
                    .then(|| bytes.len().checked_sub(4).and_then(|at| bytes.get(at..at + 4)))
                    .flatten()
                    .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                Ok(Self {
                    frames,
                    frame_rate: 50,
                    clock: 2_000_000,
                    loop_frame,
                    title: String::new(),
                    author: String::new(),
                    comment: String::new(),
                })
            }
            Some(b"YM5!" | b"YM6!") => {
                if bytes.get(4..12) != Some(b"LeOnArD!") {
                    return Err("YM5/YM6 file lacks its signature".to_owned());
                }
                let count = u32_at(12).ok_or("truncated YM header")? as usize;
                let attributes = u32_at(16).ok_or("truncated YM header")?;
                let drums = u16_at(20).ok_or("truncated YM header")?;
                let clock = u32_at(22).ok_or("truncated YM header")?;
                let frame_rate = u32::from(u16_at(26).ok_or("truncated YM header")?);
                let loop_frame = u32_at(28).ok_or("truncated YM header")?;
                let extra = usize::from(u16_at(32).ok_or("truncated YM header")?);
                let mut at = 34 + extra;
                for _ in 0..drums {
                    let size = u32_at(at).ok_or("truncated YM digidrum")? as usize;
                    at = at.checked_add(4 + size).ok_or("YM digidrum overflow")?;
                }
                let mut text = || -> Result<String, String> {
                    let rest = bytes.get(at..).ok_or("truncated YM text")?;
                    let end = rest.iter().position(|b| *b == 0).ok_or("unterminated YM text")?;
                    at += end + 1;
                    Ok(kog_core::text_encoding::decode(&rest[..end]).trim().to_owned())
                };
                let (title, author, comment) = (text()?, text()?, text()?);
                if count == 0 || frame_rate == 0 || clock == 0 {
                    return Err("YM file has no frames".to_owned());
                }
                let data = bytes
                    .get(at..at.checked_add(count.checked_mul(16).ok_or("YM frame count overflow")?).ok_or("YM overflow")?)
                    .ok_or("YM register data is truncated")?;
                let interleaved = attributes & 1 != 0;
                let frames = (0..count)
                    .map(|frame| {
                        let mut registers = [0_u8; 16];
                        for (register, value) in registers.iter_mut().enumerate() {
                            *value = if interleaved { data[register * count + frame] } else { data[frame * 16 + register] };
                        }
                        registers
                    })
                    .collect();
                Ok(Self {
                    frames,
                    frame_rate,
                    clock,
                    loop_frame: (loop_frame > 0 && (loop_frame as usize) < count).then_some(loop_frame),
                    title,
                    author,
                    comment,
                })
            }
            _ => Err("not a YM2/YM3/YM5/YM6 register dump".to_owned()),
        }
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.frames.len() as f64 / f64::from(self.frame_rate))
    }
}

/// Renders a song once through at `sample_rate`, interleaved stereo.
pub struct YmPlayer {
    chip: NonNull<c_void>,
    song: YmSong,
    sample_rate: u32,
    frame: usize,
    /// Output frames left before the next register frame.
    until_next: f64,
}

unsafe impl Send for YmPlayer {}

impl YmPlayer {
    pub fn new(song: YmSong, sample_rate: u32) -> Result<Self, String> {
        // Atari ST clocks are 2 MHz; CPC is 1 MHz and Spectrum ~1.77 MHz (AY).
        let atari = song.clock == 2_000_000;
        let chip = NonNull::new(unsafe {
            kog_ayumi_new(i32::from(atari), f64::from(song.clock), sample_rate as i32, i32::from(!atari))
        })
        .ok_or("ayumi rejected the YM clock rate")?;
        Ok(Self { chip, song, sample_rate, frame: 0, until_next: 0.0 })
    }

    pub fn song(&self) -> &YmSong {
        &self.song
    }

    pub fn total_frames(&self) -> u64 {
        (self.song.duration().as_secs_f64() * f64::from(self.sample_rate)) as u64
    }

    /// Fill `output` with stereo frames; returns how many (0 at the end).
    pub fn render(&mut self, output: &mut [f32]) -> usize {
        let per_frame = f64::from(self.sample_rate) / f64::from(self.song.frame_rate);
        let wanted = output.len() / 2;
        let mut written = 0;
        while written < wanted {
            if self.until_next <= 0.0 {
                let Some(registers) = self.song.frames.get(self.frame) else { break };
                unsafe { kog_ayumi_set_registers(self.chip.as_ptr(), registers.as_ptr()) };
                self.frame += 1;
                self.until_next += per_frame;
            }
            let run = (self.until_next.ceil() as usize).clamp(1, wanted - written);
            unsafe { kog_ayumi_render(self.chip.as_ptr(), output[written * 2..].as_mut_ptr(), run) };
            written += run;
            self.until_next -= run as f64;
        }
        written
    }

    pub fn seek(&mut self, position: Duration) {
        let frame = (position.as_secs_f64() * f64::from(self.song.frame_rate)) as usize;
        self.frame = frame.min(self.song.frames.len());
        self.until_next = 0.0;
        // Restore the envelope shape in effect, which later frames only
        // repeat as "unchanged".
        if let Some(shape) = self.song.frames[..self.frame].iter().rev().map(|r| r[13]).find(|shape| *shape != 0xff)
            && let Some(mut registers) = self.song.frames.get(self.frame).copied()
        {
            registers[13] = shape;
            unsafe { kog_ayumi_set_registers(self.chip.as_ptr(), registers.as_ptr()) };
        }
    }
}

impl Drop for YmPlayer {
    fn drop(&mut self) {
        unsafe { kog_ayumi_free(self.chip.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ym3_frames_deinterleave_and_render() {
        // Two frames: channel A tone on at volume 15, then silent.
        let mut bytes = b"YM3!".to_vec();
        let registers: [[u8; 14]; 2] = [
            [0x40, 0, 0, 0, 0, 0, 0, 0b0011_1110, 15, 0, 0, 0, 0, 0xff],
            [0x40, 0, 0, 0, 0, 0, 0, 0b0011_1111, 0, 0, 0, 0, 0, 0xff],
        ];
        for register in 0..14 {
            for frame in &registers {
                bytes.push(frame[register]);
            }
        }
        let song = YmSong::parse(&bytes).unwrap();
        assert_eq!(song.frames.len(), 2);
        assert_eq!(song.frames[0][8], 15);
        assert_eq!(song.duration(), Duration::from_millis(40));
        let mut player = YmPlayer::new(song, 44_100).unwrap();
        let mut pcm = vec![0.0_f32; 2 * 4_096];
        let frames = player.render(&mut pcm);
        assert!(frames >= 1_700, "renders both register frames ({frames})");
        assert!(pcm[..882 * 2].iter().any(|sample| sample.abs() > 0.01), "the tone sounds");
    }
}

#[cfg(test)]
mod probe {
    use super::*;

    /// Parses and renders real YM files:
    ///   KOG_YM_PROBE="<a.ym>|<b.ym>" cargo test -p kog-audio --lib -- --ignored ym_probe --nocapture
    #[test]
    #[ignore = "needs KOG_YM_PROBE"]
    fn ym_probe() {
        for path in std::env::var("KOG_YM_PROBE").expect("set KOG_YM_PROBE").split('|') {
            match YmSong::open(Path::new(path)) {
                Ok(song) => {
                    let (frames, clock, rate, title) = (song.frames.len(), song.clock, song.frame_rate, song.title.clone());
                    let duration = song.duration();
                    let mut player = YmPlayer::new(song, 44_100).unwrap();
                    let mut pcm = vec![0.0_f32; 2 * 44_100 * 10];
                    let rendered = player.render(&mut pcm);
                    let peak = pcm[..rendered * 2].iter().fold(0.0_f32, |p, s| p.max(s.abs()));
                    println!("ok {frames} frames {clock} Hz {rate} fps {duration:?} peak {peak:.3} '{title}' {path}");
                }
                Err(error) => println!("FAIL {error}"),
            }
        }
    }
}
