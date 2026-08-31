//! Safe ownership wrapper around libopenmpt's stable C API.

use std::collections::BTreeMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

const RENDER_MASTER_GAIN_MILLIBEL: c_int = 1;
const RENDER_STEREO_SEPARATION_PERCENT: c_int = 2;
const RENDER_INTERPOLATION_FILTER_LENGTH: c_int = 3;
const RENDER_VOLUME_RAMPING_STRENGTH: c_int = 4;

#[repr(C)]
struct OpenMptModule {
    _private: [u8; 0],
}

#[repr(C)]
struct OpenMptInitialCtl {
    ctl: *const c_char,
    value: *const c_char,
}

type OpenMptLogFunc = Option<unsafe extern "C" fn(*const c_char, *mut c_void)>;
type OpenMptErrorFunc = Option<unsafe extern "C" fn(c_int, *mut c_void) -> c_int>;

unsafe extern "C" {
    fn openmpt_free_string(value: *const c_char);
    #[cfg(test)]
    fn openmpt_get_supported_extensions() -> *const c_char;
    fn openmpt_is_extension_supported(extension: *const c_char) -> c_int;
    fn openmpt_log_func_silent(message: *const c_char, user: *mut c_void);
    fn openmpt_error_func_ignore(error: c_int, user: *mut c_void) -> c_int;
    fn openmpt_error_string(error: c_int) -> *const c_char;
    fn openmpt_module_create_from_memory2(
        data: *const c_void,
        size: usize,
        log_func: OpenMptLogFunc,
        log_user: *mut c_void,
        error_func: OpenMptErrorFunc,
        error_user: *mut c_void,
        error: *mut c_int,
        error_message: *mut *const c_char,
        ctls: *const OpenMptInitialCtl,
    ) -> *mut OpenMptModule;
    fn openmpt_module_destroy(module: *mut OpenMptModule);
    fn openmpt_module_error_get_last(module: *mut OpenMptModule) -> c_int;
    fn openmpt_module_error_clear(module: *mut OpenMptModule);
    fn openmpt_module_get_num_subsongs(module: *mut OpenMptModule) -> i32;
    fn openmpt_module_select_subsong(module: *mut OpenMptModule, subsong: i32) -> c_int;
    fn openmpt_module_set_repeat_count(module: *mut OpenMptModule, count: i32) -> c_int;
    fn openmpt_module_get_duration_seconds(module: *mut OpenMptModule) -> f64;
    fn openmpt_module_set_position_seconds(module: *mut OpenMptModule, seconds: f64) -> f64;
    fn openmpt_module_set_render_param(
        module: *mut OpenMptModule,
        parameter: c_int,
        value: i32,
    ) -> c_int;
    fn openmpt_module_ctl_set_boolean(
        module: *mut OpenMptModule,
        ctl: *const c_char,
        value: c_int,
    ) -> c_int;
    fn openmpt_module_read_interleaved_float_stereo(
        module: *mut OpenMptModule,
        sample_rate: i32,
        frames: usize,
        output: *mut f32,
    ) -> usize;
    fn openmpt_module_get_metadata_keys(module: *mut OpenMptModule) -> *const c_char;
    fn openmpt_module_get_metadata(module: *mut OpenMptModule, key: *const c_char)
    -> *const c_char;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenMptMetadata {
    values: BTreeMap<String, String>,
}

impl OpenMptMetadata {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }
}

pub struct OpenMpt {
    handle: NonNull<OpenMptModule>,
    sample_rate: u32,
    duration: Duration,
    subsongs: u32,
    selected_subsong: u32,
    metadata: OpenMptMetadata,
}

impl OpenMpt {
    pub fn open(path: &Path, sample_rate: u32, subsong: Option<u32>) -> Result<Self, String> {
        let data =
            std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
        let initial_ctls = [
            OpenMptInitialCtl {
                ctl: c"seek.sync_samples".as_ptr(),
                value: c"1".as_ptr(),
            },
            OpenMptInitialCtl {
                ctl: std::ptr::null(),
                value: std::ptr::null(),
            },
        ];
        let mut error = 0;
        let mut error_message = std::ptr::null();
        let handle = NonNull::new(unsafe {
            openmpt_module_create_from_memory2(
                data.as_ptr().cast(),
                data.len(),
                Some(openmpt_log_func_silent),
                std::ptr::null_mut(),
                Some(openmpt_error_func_ignore),
                std::ptr::null_mut(),
                &mut error,
                &mut error_message,
                initial_ctls.as_ptr(),
            )
        });
        let create_message = take_native_string(error_message);
        let Some(handle) = handle else {
            let message = create_message
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| error_text(error));
            return Err(format!(
                "opening {} with libopenmpt: {message}",
                path.display()
            ));
        };

        let mut module = Self {
            handle,
            sample_rate,
            duration: Duration::ZERO,
            subsongs: 0,
            selected_subsong: subsong.unwrap_or(0),
            metadata: OpenMptMetadata::default(),
        };
        let subsongs = unsafe { openmpt_module_get_num_subsongs(module.handle.as_ptr()) };
        module.subsongs =
            u32::try_from(subsongs).map_err(|_| module.fail("querying libopenmpt subsongs"))?;
        if module.subsongs == 0 || module.selected_subsong >= module.subsongs {
            return Err(format!(
                "{} requests OpenMPT subsong {}, but the file contains {}",
                path.display(),
                module.selected_subsong + 1,
                module.subsongs
            ));
        }
        if unsafe {
            openmpt_module_select_subsong(
                module.handle.as_ptr(),
                i32::try_from(module.selected_subsong).expect("OpenMPT subsong fits i32"),
            )
        } == 0
        {
            return Err(module.fail("selecting libopenmpt subsong"));
        }
        module.require(
            unsafe { openmpt_module_set_repeat_count(module.handle.as_ptr(), 0) },
            "setting libopenmpt repeat policy",
        )?;
        for (parameter, value, description) in [
            (RENDER_MASTER_GAIN_MILLIBEL, 0, "master gain"),
            (RENDER_STEREO_SEPARATION_PERCENT, 100, "stereo separation"),
            (RENDER_INTERPOLATION_FILTER_LENGTH, 8, "interpolation"),
            (RENDER_VOLUME_RAMPING_STRENGTH, -1, "volume ramping"),
        ] {
            module.require(
                unsafe {
                    openmpt_module_set_render_param(module.handle.as_ptr(), parameter, value)
                },
                &format!("setting libopenmpt {description}"),
            )?;
        }
        module.require(
            unsafe {
                openmpt_module_ctl_set_boolean(
                    module.handle.as_ptr(),
                    c"render.resampler.emulate_amiga".as_ptr(),
                    1,
                )
            },
            "enabling libopenmpt Amiga resampler emulation",
        )?;

        let seconds = unsafe { openmpt_module_get_duration_seconds(module.handle.as_ptr()) };
        module.duration = duration_from_seconds(seconds)?;
        module.metadata = module.read_metadata();
        Ok(module)
    }

    #[cfg(test)]
    pub fn supported_extensions() -> Vec<String> {
        take_native_string(unsafe { openmpt_get_supported_extensions() })
            .unwrap_or_default()
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(str::to_owned)
            .collect()
    }

    pub fn supports_extension(extension: &str) -> bool {
        let Ok(extension) = CString::new(extension) else {
            return false;
        };
        unsafe { openmpt_is_extension_supported(extension.as_ptr()) != 0 }
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn subsong_count(&self) -> u32 {
        self.subsongs
    }

    pub fn selected_subsong(&self) -> u32 {
        self.selected_subsong
    }

    pub fn metadata(&self) -> &OpenMptMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        if !output.len().is_multiple_of(2) {
            return Err("libopenmpt output must contain stereo sample pairs".to_owned());
        }
        unsafe { openmpt_module_error_clear(self.handle.as_ptr()) };
        let frames = unsafe {
            openmpt_module_read_interleaved_float_stereo(
                self.handle.as_ptr(),
                i32::try_from(self.sample_rate).expect("OpenMPT sample rate fits i32"),
                output.len() / 2,
                output.as_mut_ptr(),
            )
        };
        self.check_last_error("rendering libopenmpt audio")?;
        Ok(frames)
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        unsafe { openmpt_module_error_clear(self.handle.as_ptr()) };
        let actual = unsafe {
            openmpt_module_set_position_seconds(
                self.handle.as_ptr(),
                position.min(self.duration).as_secs_f64(),
            )
        };
        self.check_last_error("seeking libopenmpt audio")?;
        duration_from_seconds(actual)
    }

    fn read_metadata(&self) -> OpenMptMetadata {
        let keys =
            take_native_string(unsafe { openmpt_module_get_metadata_keys(self.handle.as_ptr()) })
                .unwrap_or_default();
        let mut values = BTreeMap::new();
        for key in keys
            .split(';')
            .filter(|key| !key.is_empty() && *key != "type")
        {
            let Ok(native_key) = CString::new(key) else {
                continue;
            };
            let value = take_native_string(unsafe {
                openmpt_module_get_metadata(self.handle.as_ptr(), native_key.as_ptr())
            })
            .unwrap_or_default();
            values.insert(key.to_owned(), value);
        }
        OpenMptMetadata { values }
    }

    fn require(&self, result: c_int, action: &str) -> Result<(), String> {
        if result == 0 {
            Err(self.fail(action))
        } else {
            Ok(())
        }
    }

    fn check_last_error(&self, action: &str) -> Result<(), String> {
        let error = unsafe { openmpt_module_error_get_last(self.handle.as_ptr()) };
        if error == 0 {
            Ok(())
        } else {
            Err(format!("{action}: {}", error_text(error)))
        }
    }

    fn fail(&self, action: &str) -> String {
        let error = unsafe { openmpt_module_error_get_last(self.handle.as_ptr()) };
        format!("{action}: {}", error_text(error))
    }
}

unsafe impl Send for OpenMpt {}

impl Drop for OpenMpt {
    fn drop(&mut self) {
        unsafe { openmpt_module_destroy(self.handle.as_ptr()) };
    }
}

fn duration_from_seconds(seconds: f64) -> Result<Duration, String> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(format!("libopenmpt returned invalid duration {seconds}"));
    }
    Duration::try_from_secs_f64(seconds)
        .map_err(|_| format!("libopenmpt duration {seconds} exceeds Kog's limit"))
}

fn error_text(error: c_int) -> String {
    if error == 0 {
        return "native libopenmpt operation failed".to_owned();
    }
    take_native_string(unsafe { openmpt_error_string(error) })
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| format!("libopenmpt error {error}"))
}

fn take_native_string(value: *const c_char) -> Option<String> {
    let value = NonNull::new(value.cast_mut())?;
    let result = unsafe { CStr::from_ptr(value.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    unsafe { openmpt_free_string(value.as_ptr()) };
    Some(result)
}

#[cfg(test)]
pub fn test_mod_bytes() -> Vec<u8> {
    let mut module = Vec::with_capacity(2_108 + 32);
    let mut title = [0_u8; 20];
    title[..16].copy_from_slice(b"Kog OpenMPT Test");
    module.extend_from_slice(&title);

    for sample in 0..31 {
        let mut header = [0_u8; 30];
        if sample == 0 {
            header[..8].copy_from_slice(b"Kog tone");
            header[22..24].copy_from_slice(&16_u16.to_be_bytes());
            header[25] = 64;
            header[28..30].copy_from_slice(&16_u16.to_be_bytes());
        }
        module.extend_from_slice(&header);
    }
    module.push(1);
    module.push(0);
    module.extend(std::iter::repeat_n(0_u8, 128));
    module.extend_from_slice(b"M.K.");

    let mut pattern = [0_u8; 1_024];
    pattern[0..4].copy_from_slice(&[0x01, 0xAC, 0x10, 0x00]);
    module.extend_from_slice(&pattern);
    module.extend_from_slice(&[
        0, 48, 88, 117, 127, 117, 88, 48, 0, 208, 168, 139, 128, 139, 168, 208, 0, 48, 88, 117,
        127, 117, 88, 48, 0, 208, 168, 139, 128, 139, 168, 208,
    ]);
    module
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_mod_renders_metadata_non_silent_pcm_and_seeks() {
        let path =
            std::env::temp_dir().join(format!("kog-openmpt-core-{}.mod", std::process::id()));
        std::fs::write(&path, test_mod_bytes()).expect("write MOD fixture");
        let mut decoder = OpenMpt::open(&path, 44_100, Some(0)).expect("open MOD fixture");

        assert_eq!(decoder.subsong_count(), 1);
        assert_eq!(decoder.selected_subsong(), 0);
        assert_eq!(decoder.metadata().get("title"), Some("Kog OpenMPT Test"));
        assert_eq!(
            decoder.metadata().get("type_long"),
            Some("ProTracker MOD (M.K.)")
        );
        assert!(decoder.duration() > Duration::from_secs(7));
        let mut pcm = vec![0.0_f32; 4_410 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render MOD"), 4_410);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        assert!(decoder.seek(Duration::from_secs(1)).is_ok());
        pcm.fill(0.0);
        assert_eq!(decoder.render(&mut pcm).expect("render after seek"), 4_410);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));
        assert!(OpenMpt::supports_extension("mod"));
        assert!(
            OpenMpt::supported_extensions()
                .iter()
                .any(|ext| ext == "it")
        );

        std::fs::remove_file(path).ok();
    }
}
