//! Safe ownership wrapper around Kog's bridge to Furnace's engine, which
//! plays FamiTracker (.ftm, .0cc, .dnm), Furnace (.fur) and DefleMask (.dmf)
//! modules. The bridge reports how long the song takes to loop the requested
//! number of times; Kog fades out after that, as for its other trackers.

use std::ffi::{CStr, c_char, c_void};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

unsafe extern "C" {
    fn kog_furnace_open(
        data: *const u8,
        size: usize,
        name: *const c_char,
        rate: u32,
        loops: u32,
        error: *mut c_char,
        capacity: usize,
    ) -> *mut c_void;
    fn kog_furnace_subsong_count(handle: *mut c_void) -> u32;
    fn kog_furnace_text(handle: *mut c_void, which: u32) -> *const c_char;
    fn kog_furnace_start(handle: *mut c_void, subsong: u32) -> u64;
    fn kog_furnace_render(handle: *mut c_void, output: *mut f32, frames: usize) -> usize;
    fn kog_furnace_close(handle: *mut c_void);
}

pub struct Furnace {
    handle: NonNull<c_void>,
    data: Vec<u8>,
    name: std::ffi::CString,
    sample_rate: u32,
    loops: u32,
    subsong: u32,
    subsong_count: u32,
    main_frames: u64,
    fade_frames: u64,
    rendered: u64,
    title: String,
    author: String,
    album: String,
    system: String,
}

impl Furnace {
    pub fn open(
        path: &Path,
        sample_rate: u32,
        subsong: Option<u32>,
        loops: u32,
        fade: Duration,
    ) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
        let name = std::ffi::CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| format!("{} contains a NUL byte", path.display()))?;
        let handle = open_handle(&data, &name, sample_rate, loops)?;
        let subsong_count = unsafe { kog_furnace_subsong_count(handle.as_ptr()) };
        let text = |which| unsafe {
            let value = kog_furnace_text(handle.as_ptr(), which);
            if value.is_null() { String::new() } else { CStr::from_ptr(value).to_string_lossy().trim().to_owned() }
        };
        let (title, author, album, system) = (text(0), text(1), text(2), text(3));
        let subsong = subsong.unwrap_or(0);
        let mut furnace = Self {
            handle,
            data,
            name,
            sample_rate,
            loops,
            subsong,
            subsong_count,
            main_frames: 0,
            fade_frames: (fade.as_secs_f64() * f64::from(sample_rate)) as u64,
            rendered: 0,
            title,
            author,
            album,
            system,
        };
        if subsong >= subsong_count {
            return Err(format!(
                "{} requests subsong {}, but the module has {subsong_count}",
                path.display(),
                subsong + 1
            ));
        }
        furnace.main_frames = unsafe { kog_furnace_start(furnace.handle.as_ptr(), subsong) };
        if furnace.main_frames == 0 {
            return Err(format!("Furnace could not start {}", path.display()));
        }
        Ok(furnace)
    }

    pub fn subsong_count(&self) -> u32 {
        self.subsong_count
    }

    pub fn subsong(&self) -> u32 {
        self.subsong
    }

    /// Song title, then the subsong's own name when there are several.
    pub fn title(&self) -> String {
        let subsong = unsafe { kog_furnace_text(self.handle.as_ptr(), 4) };
        let subsong = if subsong.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(subsong) }.to_string_lossy().trim().to_owned()
        };
        match (self.title.is_empty(), subsong.is_empty() || self.subsong_count < 2) {
            (_, true) => self.title.clone(),
            (true, false) => subsong,
            (false, false) => format!("{} - {subsong}", self.title),
        }
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn album(&self) -> &str {
        &self.album
    }

    pub fn system(&self) -> &str {
        &self.system
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64((self.main_frames + self.fade_frames) as f64 / f64::from(self.sample_rate))
    }

    /// Render interleaved stereo frames, fading out after the loops end.
    pub fn render(&mut self, output: &mut [f32]) -> usize {
        let total = self.main_frames + self.fade_frames;
        let wanted = (output.len() / 2).min(total.saturating_sub(self.rendered) as usize);
        if wanted == 0 {
            return 0;
        }
        let got = unsafe { kog_furnace_render(self.handle.as_ptr(), output.as_mut_ptr(), wanted) };
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

    /// Restart and render ahead to `position` (the engine cannot jump).
    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = ((position.as_secs_f64() * f64::from(self.sample_rate)) as u64)
            .min(self.main_frames + self.fade_frames);
        if target < self.rendered {
            let handle = open_handle(&self.data, &self.name, self.sample_rate, self.loops)?;
            unsafe { kog_furnace_close(self.handle.as_ptr()) };
            self.handle = handle;
            self.main_frames = unsafe { kog_furnace_start(self.handle.as_ptr(), self.subsong) };
            self.rendered = 0;
        }
        let mut scratch = vec![0.0_f32; 4096 * 2];
        while self.rendered < target {
            let frames = ((target - self.rendered) as usize).min(4096);
            let got = unsafe { kog_furnace_render(self.handle.as_ptr(), scratch.as_mut_ptr(), frames) };
            if got == 0 {
                break;
            }
            self.rendered += got as u64;
        }
        Ok(Duration::from_secs_f64(self.rendered as f64 / f64::from(self.sample_rate)))
    }
}

fn open_handle(data: &[u8], name: &std::ffi::CStr, rate: u32, loops: u32) -> Result<NonNull<c_void>, String> {
    let mut error = vec![0 as c_char; 512];
    let handle = unsafe {
        kog_furnace_open(data.as_ptr(), data.len(), name.as_ptr(), rate, loops, error.as_mut_ptr(), error.len())
    };
    NonNull::new(handle).ok_or_else(|| {
        let message = unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy().into_owned();
        if message.is_empty() { "Furnace could not open the module".to_owned() } else { message }
    })
}

unsafe impl Send for Furnace {}

impl Drop for Furnace {
    fn drop(&mut self) {
        unsafe { kog_furnace_close(self.handle.as_ptr()) };
    }
}

/// Whether a file's first bytes are a module Furnace plays, so `.ftm` Face
/// The Music modules and X-Tracker `.dmf` files stay with OpenMPT.
pub fn is_furnace_module(path: &Path) -> bool {
    use std::io::Read;
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    if matches!(extension.as_str(), "fur" | "0cc" | "dnm") {
        return true;
    }
    let mut head = [0_u8; 24];
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let Ok(read) = file.read(&mut head) else { return false };
    let head = &head[..read];
    match extension.as_str() {
        "ftm" => head.starts_with(b"FamiTracker Module") || head.starts_with(b"Dn-FamiTracker Module")
            || head.starts_with(b"0CC-FamiTracker Module"),
        // DefleMask modules are zlib-compressed (0x78); X-Tracker's start "DDMF".
        "dmf" => head.first() == Some(&0x78) || head.starts_with(b".DelekDefleMask."),
        _ => false,
    }
}
