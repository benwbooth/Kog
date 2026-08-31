//! Safe ownership and metadata wrapper around libGME's C API.

use std::ffi::{CStr, CString, c_char, c_int, c_long, c_void};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::time::Duration;

unsafe extern "C" {
    fn gme_identify_extension(path_or_extension: *const c_char) -> *const c_void;
    fn gme_new_emu(music_type: *const c_void, sample_rate: c_int) -> *mut c_void;
    fn gme_load_data(emu: *mut c_void, data: *const c_void, size: c_long) -> *const c_char;
    fn gme_load_m3u_data(emu: *mut c_void, data: *const c_void, size: c_long) -> *const c_char;
    fn gme_track_count(emu: *const c_void) -> c_int;
    fn gme_track_info(
        emu: *const c_void,
        output: *mut *mut GmeInfoRaw,
        track: c_int,
    ) -> *const c_char;
    fn gme_free_info(info: *mut GmeInfoRaw);
    fn gme_start_track(emu: *mut c_void, track: c_int) -> *const c_char;
    fn gme_set_fade_msecs(emu: *mut c_void, start_msec: c_int, length_msec: c_int);
    fn gme_play(emu: *mut c_void, count: c_int, output: *mut i16) -> *const c_char;
    fn gme_track_ended(emu: *const c_void) -> c_int;
    fn gme_seek(emu: *mut c_void, milliseconds: c_int) -> *const c_char;
    fn gme_warning(emu: *mut c_void) -> *const c_char;
    fn gme_delete(emu: *mut c_void);
}

#[repr(C)]
struct GmeInfoRaw {
    length: c_int,
    intro_length: c_int,
    loop_length: c_int,
    play_length: c_int,
    fade_length: c_int,
    reserved_ints: [c_int; 11],
    system: *const c_char,
    game: *const c_char,
    song: *const c_char,
    author: *const c_char,
    copyright: *const c_char,
    comment: *const c_char,
    dumper: *const c_char,
    reserved_strings: [*const c_char; 9],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GmeTrackInfo {
    pub length_ms: i32,
    pub intro_length_ms: i32,
    pub loop_length_ms: i32,
    pub fade_length_ms: i32,
    pub system: String,
    pub game: String,
    pub song: String,
    pub author: String,
    pub copyright: String,
    pub comment: String,
    pub dumper: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GmePlaybackPlan {
    pub play_length_ms: u64,
    pub fade_length_ms: u64,
    pub total_length_ms: u64,
}

impl GmeTrackInfo {
    pub fn playback_plan(
        &self,
        default_length: Duration,
        default_fade: Duration,
        loop_count: u32,
    ) -> GmePlaybackPlan {
        let play_length_ms = if self.length_ms > 0 {
            self.length_ms as u64
        } else if self.loop_length_ms > 0 {
            self.intro_length_ms.max(0) as u64
                + self.loop_length_ms as u64 * u64::from(loop_count.min(10))
        } else {
            duration_milliseconds(default_length)
        };
        let fade_length_ms = if self.fade_length_ms >= 0 {
            self.fade_length_ms as u64
        } else {
            duration_milliseconds(default_fade)
        };
        GmePlaybackPlan {
            play_length_ms,
            fade_length_ms,
            total_length_ms: play_length_ms.saturating_add(fade_length_ms),
        }
    }
}

fn duration_milliseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

pub struct GameMusicEmu {
    handle: NonNull<c_void>,
    pub warning: Option<String>,
}

impl GameMusicEmu {
    pub fn open(path: &Path, sample_rate: i32) -> Result<Self, String> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("{} has no GME file extension", path.display()))?;
        let extension = CString::new(extension)
            .map_err(|_| format!("{} has an invalid file extension", path.display()))?;
        let music_type = unsafe { gme_identify_extension(extension.as_ptr()) };
        if music_type.is_null() {
            return Err(format!(
                "Game Music Emu does not recognize {}",
                path.display()
            ));
        }
        let handle = NonNull::new(unsafe { gme_new_emu(music_type, sample_rate) })
            .ok_or_else(|| "allocating a Game Music Emu decoder failed".to_owned())?;
        let mut emu = Self {
            handle,
            warning: None,
        };
        let data =
            std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
        let size = c_long::try_from(data.len())
            .map_err(|_| format!("{} is too large for Game Music Emu", path.display()))?;
        let error =
            unsafe { gme_load_data(emu.handle.as_ptr(), data.as_ptr().cast::<c_void>(), size) };
        check_gme_error(error, format!("loading {}", path.display()))?;
        emu.warning = take_gme_warning(emu.handle, format!("loading {}", path.display()));

        let companion = companion_m3u_path(path);
        match std::fs::read(&companion) {
            Ok(data) => {
                let size = c_long::try_from(data.len()).map_err(|_| {
                    format!("{} is too large for Game Music Emu", companion.display())
                })?;
                let error = unsafe {
                    gme_load_m3u_data(emu.handle.as_ptr(), data.as_ptr().cast::<c_void>(), size)
                };
                if !error.is_null() {
                    append_warning(
                        &mut emu.warning,
                        gme_error_message(
                            error,
                            format!("loading companion {}", companion.display()),
                        ),
                    );
                } else if let Some(warning) = take_gme_warning(
                    emu.handle,
                    format!("loading companion {}", companion.display()),
                ) {
                    append_warning(&mut emu.warning, warning);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                append_warning(
                    &mut emu.warning,
                    format!("reading companion {}: {error}", companion.display()),
                );
            }
        }
        Ok(emu)
    }

    pub fn track_count(&self) -> Result<u32, String> {
        let count = unsafe { gme_track_count(self.handle.as_ptr()) };
        u32::try_from(count)
            .map_err(|_| "Game Music Emu returned a negative track count".to_owned())
    }

    pub fn track_info(&self, track: u32) -> Result<GmeTrackInfo, String> {
        let track =
            c_int::try_from(track).map_err(|_| "GME track index is too large".to_owned())?;
        let mut raw = std::ptr::null_mut();
        let error = unsafe { gme_track_info(self.handle.as_ptr(), &mut raw, track) };
        if !error.is_null() {
            if !raw.is_null() {
                unsafe { gme_free_info(raw) };
            }
            return Err(gme_error_message(
                error,
                format!("reading GME track {} metadata", track + 1),
            ));
        }
        let raw = NonNull::new(raw)
            .ok_or_else(|| "Game Music Emu returned no track metadata".to_owned())?;
        let info = unsafe { raw.as_ref() };
        let output = GmeTrackInfo {
            length_ms: info.length,
            intro_length_ms: info.intro_length,
            loop_length_ms: info.loop_length,
            fade_length_ms: info.fade_length,
            system: copy_gme_string(info.system),
            game: copy_gme_string(info.game),
            song: copy_gme_string(info.song),
            author: copy_gme_string(info.author),
            copyright: copy_gme_string(info.copyright),
            comment: copy_gme_string(info.comment),
            dumper: copy_gme_string(info.dumper),
        };
        unsafe { gme_free_info(raw.as_ptr()) };
        Ok(output)
    }

    pub fn start_track(&mut self, track: u32, plan: GmePlaybackPlan) -> Result<(), String> {
        let track =
            c_int::try_from(track).map_err(|_| "GME track index is too large".to_owned())?;
        let error = unsafe { gme_start_track(self.handle.as_ptr(), track) };
        check_gme_error(error, format!("starting GME track {}", track + 1))?;
        let fade_start = c_int::try_from(plan.play_length_ms)
            .map_err(|_| "GME track length exceeds the native API limit".to_owned())?;
        let fade_length = c_int::try_from(plan.fade_length_ms)
            .map_err(|_| "GME fade length exceeds the native API limit".to_owned())?;
        unsafe { gme_set_fade_msecs(self.handle.as_ptr(), fade_start, fade_length) };
        Ok(())
    }

    pub fn render(&mut self, output: &mut [i16]) -> Result<(), String> {
        if !output.len().is_multiple_of(2) {
            return Err("Game Music Emu output must contain stereo sample pairs".to_owned());
        }
        let count = c_int::try_from(output.len())
            .map_err(|_| "Game Music Emu output buffer is too large".to_owned())?;
        let error = unsafe { gme_play(self.handle.as_ptr(), count, output.as_mut_ptr()) };
        check_gme_error(error, "rendering Game Music Emu audio".to_owned())
    }

    pub fn track_ended(&self) -> bool {
        unsafe { gme_track_ended(self.handle.as_ptr()) != 0 }
    }

    pub fn seek(&mut self, position: Duration) -> Result<(), String> {
        let milliseconds = c_int::try_from(position.as_millis())
            .map_err(|_| "GME seek position exceeds the native API limit".to_owned())?;
        let error = unsafe { gme_seek(self.handle.as_ptr(), milliseconds) };
        check_gme_error(error, "seeking Game Music Emu audio".to_owned())
    }
}

unsafe impl Send for GameMusicEmu {}

impl Drop for GameMusicEmu {
    fn drop(&mut self) {
        unsafe { gme_delete(self.handle.as_ptr()) };
    }
}

fn companion_m3u_path(path: &Path) -> PathBuf {
    path.with_extension("m3u")
}

fn copy_gme_string(value: *const c_char) -> String {
    if value.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned()
}

fn check_gme_error(error: *const c_char, context: String) -> Result<(), String> {
    if error.is_null() {
        Ok(())
    } else {
        Err(gme_error_message(error, context))
    }
}

fn take_gme_warning(handle: NonNull<c_void>, context: String) -> Option<String> {
    let warning = unsafe { gme_warning(handle.as_ptr()) };
    (!warning.is_null()).then(|| gme_error_message(warning, context))
}

fn append_warning(slot: &mut Option<String>, warning: String) {
    match slot {
        Some(current) => {
            current.push_str("; ");
            current.push_str(&warning);
        }
        None => *slot = Some(warning),
    }
}

fn gme_error_message(error: *const c_char, context: String) -> String {
    let message = unsafe { CStr::from_ptr(error) }.to_string_lossy();
    format!("{context}: {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_nsf_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("native/game-music-emu/test.nsf")
    }

    #[test]
    fn official_nsf_fixture_loads_metadata_renders_and_seeks() {
        let mut emu = GameMusicEmu::open(&test_nsf_path(), 44_100).expect("open test NSF");
        assert_eq!(emu.track_count().expect("NSF track count"), 1);
        let info = emu.track_info(0).expect("NSF metadata");
        assert_eq!(info.song, "BGM C");
        assert_eq!(info.length_ms, 76_780);
        let plan = info.playback_plan(Duration::from_secs(150), Duration::from_secs(8), 2);
        assert_eq!(plan.total_length_ms, 84_780);
        emu.start_track(0, plan).expect("start NSF track");

        let mut pcm = vec![0_i16; 4_410 * 2];
        emu.render(&mut pcm).expect("render NSF audio");
        assert!(pcm.iter().any(|sample| *sample != 0));
        emu.seek(Duration::from_secs(1)).expect("seek NSF track");
        pcm.fill(0);
        emu.render(&mut pcm).expect("render NSF audio after seek");
        assert!(pcm.iter().any(|sample| *sample != 0));
    }

    #[test]
    fn malformed_companion_playlist_is_reported_without_hiding_the_audio() {
        let path = std::env::temp_dir().join(format!("kog-gme-warning-{}.nsf", std::process::id()));
        let companion = path.with_extension("m3u");
        std::fs::copy(test_nsf_path(), &path).expect("copy NSF fixture");
        std::fs::write(&companion, b"# comment without any tracks\n")
            .expect("write malformed M3U fixture");

        let emu = GameMusicEmu::open(&path, -1).expect("audio remains usable");
        let warning = emu.warning.as_deref().expect("malformed companion warning");
        assert!(warning.contains("Not an m3u playlist"), "{warning}");

        std::fs::remove_file(companion).ok();
        std::fs::remove_file(path).ok();
    }
}
