//! Atari ST SNDH and sc68 music through sc68 (GPL-3.0-or-later), which
//! emulates the ST's 68000, YM2149, MFP timers and STE DMA sound.

use std::ffi::{CStr, c_char, c_void};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

unsafe extern "C" {
    fn kog_sc68_open(data: *const c_void, size: usize, rate: u32, track: i32, error: *mut c_char, error_size: usize) -> *mut c_void;
    fn kog_sc68_tracks(song: *const c_void) -> i32;
    fn kog_sc68_time_ms(song: *const c_void) -> u32;
    fn kog_sc68_text(song: *const c_void, which: i32) -> *const c_char;
    fn kog_sc68_render(song: *mut c_void, output: *mut f32, frames: i32) -> std::ffi::c_long;
    fn kog_sc68_close(song: *mut c_void);
    fn kog_sc68_last_error(song: *const c_void) -> *const c_char;
}

/// Most SNDH files carry no length; sc68's time database knows many.
const DEFAULT_LENGTH: Duration = Duration::from_secs(180);

pub struct Sndh {
    handle: NonNull<c_void>,
    data: Vec<u8>,
    sample_rate: u32,
    track: u32,
    rendered: u64,
    main_frames: u64,
    fade_frames: u64,
    has_length: bool,
}

unsafe impl Send for Sndh {}

impl Sndh {
    pub fn open(path: &Path, sample_rate: u32, track: Option<u32>, fade: Duration) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
        let track = track.unwrap_or(0);
        let handle = open_handle(&data, sample_rate, track).map_err(|error| format!("{}: {error}", path.display()))?;
        let time_ms = unsafe { kog_sc68_time_ms(handle.as_ptr()) };
        let has_length = time_ms > 0;
        let length = if has_length { Duration::from_millis(u64::from(time_ms)) } else { DEFAULT_LENGTH };
        Ok(Self {
            handle,
            data,
            sample_rate,
            track,
            rendered: 0,
            main_frames: (length.as_secs_f64() * f64::from(sample_rate)) as u64,
            fade_frames: (fade.as_secs_f64() * f64::from(sample_rate)) as u64,
            has_length,
        })
    }

    pub fn track_count(&self) -> u32 {
        unsafe { kog_sc68_tracks(self.handle.as_ptr()) }.max(1) as u32
    }

    pub fn text(&self, which: i32) -> String {
        let text = unsafe { kog_sc68_text(self.handle.as_ptr(), which) };
        if text.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(text) }.to_string_lossy().trim().to_owned()
    }

    pub fn track(&self) -> u32 {
        self.track
    }

    pub fn has_length(&self) -> bool {
        self.has_length
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64((self.main_frames + self.fade_frames) as f64 / f64::from(self.sample_rate))
    }

    /// Interleaved stereo frames; fades out over the last `fade` seconds.
    pub fn render(&mut self, output: &mut [f32]) -> usize {
        let total = self.main_frames + self.fade_frames;
        let wanted = (output.len() / 2).min(total.saturating_sub(self.rendered) as usize);
        if wanted == 0 {
            return 0;
        }
        let got = unsafe { kog_sc68_render(self.handle.as_ptr(), output.as_mut_ptr(), wanted as i32) };
        if got < 0 {
            let error = unsafe { CStr::from_ptr(kog_sc68_last_error(self.handle.as_ptr())) }.to_string_lossy();
            eprintln!("Kog SNDH playback stopped: {error}");
        }
        let got = usize::try_from(got).unwrap_or(0).min(wanted);
        for frame in 0..got {
            let position = self.rendered + frame as u64;
            if position >= self.main_frames && self.fade_frames > 0 {
                let gain = 1.0 - (position - self.main_frames) as f32 / self.fade_frames as f32;
                output[frame * 2] *= gain;
                output[frame * 2 + 1] *= gain;
            }
        }
        self.rendered += got as u64;
        got
    }

    /// Restart and render forward to `position`: the emulator cannot jump.
    pub fn seek(&mut self, position: Duration) -> Result<(), String> {
        let target = ((position.as_secs_f64() * f64::from(self.sample_rate)) as u64).min(self.main_frames + self.fade_frames);
        if target < self.rendered {
            let handle = open_handle(&self.data, self.sample_rate, self.track)?;
            unsafe { kog_sc68_close(self.handle.as_ptr()) };
            self.handle = handle;
            self.rendered = 0;
        }
        let mut scratch = vec![0.0_f32; 8_192];
        while self.rendered < target {
            let frames = ((target - self.rendered) as usize).min(4_096);
            if self.render(&mut scratch[..frames * 2]) == 0 {
                break;
            }
        }
        Ok(())
    }
}

fn open_handle(data: &[u8], rate: u32, track: u32) -> Result<NonNull<c_void>, String> {
    let mut error = vec![0 as c_char; 256];
    let handle = unsafe {
        kog_sc68_open(data.as_ptr().cast(), data.len(), rate, track as i32, error.as_mut_ptr(), error.len())
    };
    NonNull::new(handle).ok_or_else(|| unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy().into_owned())
}

impl Drop for Sndh {
    fn drop(&mut self) {
        unsafe { kog_sc68_close(self.handle.as_ptr()) };
    }
}

#[cfg(test)]
mod probe {
    use super::*;

    /// Opens and renders real SNDH/sc68 files:
    ///   KOG_SNDH_PROBE="<a.sndh>|<b.sc68>" cargo test -p kog-audio --lib -- --ignored sndh_probe --nocapture
    #[test]
    #[ignore = "needs KOG_SNDH_PROBE"]
    fn sndh_probe() {
        for path in std::env::var("KOG_SNDH_PROBE").expect("set KOG_SNDH_PROBE").split('|') {
            match Sndh::open(Path::new(path), 44_100, None, Duration::from_secs(8)) {
                Ok(mut song) => {
                    let mut pcm = vec![0.0_f32; 2 * 4_096];
                    let (mut frames, mut peak, mut calls) = (0, 0.0_f32, 0);
                    while frames < 44_100 * 10 && calls < 10_000 {
                        calls += 1;
                        let got = song.render(&mut pcm);
                        if got == 0 { break; }
                        frames += got;
                        peak = pcm[..got * 2].iter().fold(peak, |p, s| p.max(s.abs()));
                    }
                    println!("rendered {frames} frames in {calls} calls");
                    println!("ok {} tracks, {:?}{}, '{}' by '{}' [{}] peak {peak:.3} {path}", song.track_count(), song.duration(),
                        if song.has_length() { "" } else { " (default)" }, song.text(0), song.text(1), song.text(3));
                }
                Err(error) => println!("FAIL {error}"),
            }
        }
    }
}
