//! Safe ownership wrapper for Kog's FFmpeg decoder bridge.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

#[repr(C)]
struct NativeFfmpeg {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_ffmpeg_open(path: *const c_char) -> *mut NativeFfmpeg;
    fn kog_ffmpeg_close(decoder: *mut NativeFfmpeg);
    fn kog_ffmpeg_error(decoder: *const NativeFfmpeg) -> *const c_char;
    #[cfg(test)]
    fn kog_ffmpeg_version() -> *const c_char;
    fn kog_ffmpeg_codec(decoder: *const NativeFfmpeg) -> *const c_char;
    fn kog_ffmpeg_title(decoder: *const NativeFfmpeg) -> *const c_char;
    fn kog_ffmpeg_artist(decoder: *const NativeFfmpeg) -> *const c_char;
    fn kog_ffmpeg_album(decoder: *const NativeFfmpeg) -> *const c_char;
    fn kog_ffmpeg_genre(decoder: *const NativeFfmpeg) -> *const c_char;
    fn kog_ffmpeg_cuesheet(decoder: *const NativeFfmpeg) -> *const c_char;
    fn kog_ffmpeg_sample_rate(decoder: *const NativeFfmpeg) -> u32;
    fn kog_ffmpeg_channels(decoder: *const NativeFfmpeg) -> u16;
    fn kog_ffmpeg_bitrate(decoder: *const NativeFfmpeg) -> u32;
    fn kog_ffmpeg_bits_per_sample(decoder: *const NativeFfmpeg) -> u8;
    fn kog_ffmpeg_year(decoder: *const NativeFfmpeg) -> u32;
    fn kog_ffmpeg_track(decoder: *const NativeFfmpeg) -> u32;
    fn kog_ffmpeg_duration(decoder: *const NativeFfmpeg) -> f64;
    fn kog_ffmpeg_render(decoder: *mut NativeFfmpeg, output: *mut f32, frames: u32) -> i32;
    fn kog_ffmpeg_seek(decoder: *mut NativeFfmpeg, seconds: f64) -> i32;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FfmpegMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub track: Option<u32>,
    pub cuesheet: Option<String>,
}

pub struct Ffmpeg {
    handle: NonNull<NativeFfmpeg>,
    sample_rate: u32,
    channels: u16,
    duration: Option<Duration>,
    bitrate: Option<u32>,
    bits_per_sample: Option<u8>,
    codec: String,
    metadata: FfmpegMetadata,
}

impl Ffmpeg {
    pub fn open(path: &Path) -> Result<Self, String> {
        let encoded_path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
            format!(
                "FFmpeg cannot open a path containing NUL: {}",
                path.display()
            )
        })?;
        let handle =
            NonNull::new(unsafe { kog_ffmpeg_open(encoded_path.as_ptr()) }).ok_or_else(|| {
                format!(
                    "opening {} with FFmpeg: {}",
                    path.display(),
                    native_error(None)
                )
            })?;

        let sample_rate = unsafe { kog_ffmpeg_sample_rate(handle.as_ptr()) };
        let channels = unsafe { kog_ffmpeg_channels(handle.as_ptr()) };
        if sample_rate == 0 || channels == 0 {
            unsafe { kog_ffmpeg_close(handle.as_ptr()) };
            return Err(format!(
                "FFmpeg reported invalid stream properties for {}",
                path.display()
            ));
        }
        let duration_seconds = unsafe { kog_ffmpeg_duration(handle.as_ptr()) };
        let duration = (duration_seconds.is_finite() && duration_seconds > 0.0)
            .then(|| Duration::from_secs_f64(duration_seconds));
        let bitrate = nonzero(unsafe { kog_ffmpeg_bitrate(handle.as_ptr()) });
        let bits_per_sample = nonzero(unsafe { kog_ffmpeg_bits_per_sample(handle.as_ptr()) });
        let codec = native_text(unsafe { kog_ffmpeg_codec(handle.as_ptr()) })
            .unwrap_or_else(|| "FFmpeg audio".to_owned());
        let metadata = FfmpegMetadata {
            title: native_text(unsafe { kog_ffmpeg_title(handle.as_ptr()) }),
            artist: native_text(unsafe { kog_ffmpeg_artist(handle.as_ptr()) }),
            album: native_text(unsafe { kog_ffmpeg_album(handle.as_ptr()) }),
            genre: native_text(unsafe { kog_ffmpeg_genre(handle.as_ptr()) }),
            year: nonzero(unsafe { kog_ffmpeg_year(handle.as_ptr()) }),
            track: nonzero(unsafe { kog_ffmpeg_track(handle.as_ptr()) }),
            cuesheet: native_text(unsafe { kog_ffmpeg_cuesheet(handle.as_ptr()) }),
        };
        Ok(Self {
            handle,
            sample_rate,
            channels,
            duration,
            bitrate,
            bits_per_sample,
            codec,
            metadata,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub fn bitrate(&self) -> Option<u32> {
        self.bitrate
    }

    pub fn bits_per_sample(&self) -> Option<u8> {
        self.bits_per_sample
    }

    pub fn codec(&self) -> &str {
        &self.codec
    }

    pub fn metadata(&self) -> &FfmpegMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "FFmpeg output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let frames = u32::try_from(output.len() / channels)
            .map_err(|_| "FFmpeg render request exceeds the native API limit".to_owned())?;
        let rendered =
            unsafe { kog_ffmpeg_render(self.handle.as_ptr(), output.as_mut_ptr(), frames) };
        usize::try_from(rendered).map_err(|_| {
            format!(
                "FFmpeg rendering failed: {}",
                native_error(Some(self.handle))
            )
        })
    }

    pub fn seek(&mut self, position: Duration) -> Result<(), String> {
        let position = self
            .duration
            .map_or(position, |duration| position.min(duration));
        let result = unsafe { kog_ffmpeg_seek(self.handle.as_ptr(), position.as_secs_f64()) };
        if result < 0 {
            return Err(format!(
                "FFmpeg seek failed: {}",
                native_error(Some(self.handle))
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn version() -> Option<String> {
        native_text(unsafe { kog_ffmpeg_version() })
    }
}

unsafe impl Send for Ffmpeg {}

impl Drop for Ffmpeg {
    fn drop(&mut self) {
        unsafe { kog_ffmpeg_close(self.handle.as_ptr()) };
    }
}

fn native_error(handle: Option<NonNull<NativeFfmpeg>>) -> String {
    native_text(unsafe {
        kog_ffmpeg_error(handle.map_or(std::ptr::null(), |handle| handle.as_ptr().cast_const()))
    })
    .unwrap_or_else(|| "unknown FFmpeg error".to_owned())
}

fn native_text(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let value = unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .trim()
        .to_owned();
    (!value.is_empty()).then_some(value)
}

fn nonzero<T>(value: T) -> Option<T>
where
    T: Copy + Default + PartialEq,
{
    (value != T::default()).then_some(value)
}

#[cfg(test)]
pub fn test_ac3_bytes() -> Vec<u8> {
    // Four 32 kHz mono AC-3 frames containing a generated 880 Hz sine. This
    // was encoded with FFmpeg's native AC-3 encoder at 32 kbit/s; no external
    // media or copyrighted source material is embedded.
    let hex = concat!(
        "0b7784ac80402f842b021bcab0fabbf32fead410afef1112f6f54b0d9792ef243d32942d4f63b66930bdc16f9561856cc1e5a9e7d566b8b3f6c8833954629adc523be283b94a6b3ad252dbd84a7c5c3a3ce067ee2881fe920002271fcc14dfc6",
        "535ab57451153da33741254032ac21fc531dad150f444f9479392028037ac213b4bed58ac1182ad847bd8202803cab707b43ed283812a0df7c7eb7a00803cab7ffafbad2fc0929f057c8197a008041ab7f3bb3bd5f4d116d398504784020cc81",
        "0b7799f580402f842903fcb80068762f9c52d87ca6356a68ac29acf673f24a8cbfb810033e4920af3fde763cdb42e4e1e2ae69955cdf24f0059fb62d00033c41e0cee7e8b348e3268e717ae90aac49236ac0062fb72900035932601eb7edf112",
        "ecde4f20fb651cc87d5531e806cfb8650003761a5fbea3f9efdef7ca2b206be2dfe82d71b9f0077fb625000495fa1feeac01ef9a0342249fd5e277095976c260003fb88d00fbb4e2107eb40df0730e763bef4263e929f1644aa800ffb6e04d16",
        "0b7721bc80402f842903fcb900044e3f9829becea6156ae8a229ace675324a4cbfba10fe5db1461b6815645c18ca6f9ebae71f88fd09dc92000d7db100fe662c3ff37e1de9c12192bc6e47ebe60c2b07f8c58011fda900fe66e827d3ac22f05a",
        "283f1dadf071efce954594ed00157dc100fe67a80de7d826f8242c5f8d2dba38dbd01542d505801701b900fe086afc0c082880352db003bda8403ad0927fe90d801801ad00fe012df60c282688772c2c7a1dbc47961004fd0104801701c46716",
        "0b77dac780402f842903fd330068762f9c5f3f7ca973dd68ac2a52f5cec24a4cbfb7d003b8c3a7d85211e3ff93f2746efa673bb9d7a44eb002a0b52100039b8b47a0780e05ae108aa487270a382ff97e6c300230b659000378d6efd89e0b26f5",
        "0c16ca3f614c90231760a8e80190b80500037b9ed820ac05a7d686e4e33fa58e1d93ff4d051000e0b541000258e6f038b00208308144edf7ef8ec883a74480f00030b655000a38eb0fe8a9fb680ffb8ee9c03a8e85f31547dcb80770b80c11d5",
    );
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("ASCII fixture hex");
            u8::from_str_radix(pair, 16).expect("valid fixture hex")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new() -> Self {
            let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "kog-ffmpeg-fixture-{}-{id}.ac3",
                std::process::id()
            ));
            std::fs::write(&path, test_ac3_bytes()).expect("write AC-3 fixture");
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn generated_ac3_opens_renders_seeks_and_ends() {
        let fixture = Fixture::new();
        let mut decoder = Ffmpeg::open(&fixture.0).expect("open generated AC-3");
        assert!(Ffmpeg::version().is_some_and(|version| !version.is_empty()));
        assert_eq!(decoder.sample_rate(), 32_000);
        assert_eq!(decoder.channels(), 1);
        assert!(decoder.codec().contains("AC-3"), "{}", decoder.codec());
        let duration = decoder.duration().expect("AC-3 duration");
        assert!(
            (Duration::from_millis(180)..=Duration::from_millis(210)).contains(&duration),
            "{duration:?}"
        );
        assert_eq!(decoder.bitrate(), Some(32_000));

        let mut pcm = vec![0.0_f32; 2_048];
        assert_eq!(decoder.render(&mut pcm).expect("render AC-3"), 2_048);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));
        decoder
            .seek(Duration::from_millis(48))
            .expect("seek generated AC-3");
        pcm.fill(0.0);
        assert!(decoder.render(&mut pcm).expect("render after seek") > 0);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        let mut frames = 0_usize;
        loop {
            let rendered = decoder.render(&mut pcm).expect("render AC-3 tail");
            frames += rendered;
            if rendered == 0 {
                break;
            }
            assert!(frames <= 32_000, "AC-3 decoder did not reach EOS");
        }
    }
}
