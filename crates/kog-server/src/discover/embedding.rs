//! Audio embeddings: what a song sounds like to a neural network trained on
//! a large collection of sounds and music (LAION's CLAP audio encoder), as a
//! 512-number vector. Songs that sound alike get vectors pointing the same
//! way, which mixes use beside the emulator-derived features.
//!
//! The model runs on ONNX Runtime, loaded at run time from its shared
//! library, so Kog builds and runs without it and gains the embeddings where
//! it is installed. The listener picks where it runs (the "embedding-backend"
//! preference): on an AMD GPU through ROCm (MIGraphX or ROCm execution
//! providers), on any GPU through WebGPU (Vulkan, DirectX 12 or Metal), on
//! the CPU, or not at all. Automatic tries the GPUs first and falls back to
//! the CPU. The model (about 120 MB) is downloaded once into Kog's data
//! folder the first time it is needed.

use std::io::Read;
use std::path::PathBuf;

use kog_audio::decoder::{DecoderSettings, PlaybackSource};
use kog_audio::streaming::PcmReader;

/// The preference that picks the backend.
pub const BACKEND_SETTING: &str = "embedding-backend";
/// Where ONNX Runtime's library is, when it is not on the library path.
pub const LIBRARY_SETTING: &str = "onnxruntime-library";
/// Which model made a vector; vectors from another are made again.
pub const MODEL_ID: &str = "clap-htsat-unfused";
pub const DIMENSIONS: usize = 512;

const MODEL_URL: &str =
    "https://huggingface.co/Xenova/clap-htsat-unfused/resolve/main/onnx/audio_model.onnx";
const MODEL_FILE: &str = "clap-htsat-unfused-audio.onnx";
/// The model file is a little over 100 MB; anything much smaller is a
/// failed download.
const MODEL_MIN_BYTES: u64 = 50 * 1024 * 1024;

const SAMPLE_RATE: usize = 48_000;
const CLIP_SAMPLES: usize = 10 * SAMPLE_RATE;
const FFT: usize = 1024;
const HOP: usize = 480;
const MELS: usize = 64;
const FRAMES: usize = CLIP_SAMPLES / HOP + 1;
const MIN_HZ: f64 = 50.0;
const MAX_HZ: f64 = 14_000.0;

/// Where the model runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// GPUs first (ROCm, CUDA, DirectML, Core ML, WebGPU), then the CPU.
    Automatic,
    /// AMD GPUs through ROCm: MIGraphX, then the ROCm provider.
    Rocm,
    /// Any GPU through WebGPU (Vulkan, DirectX 12, Metal).
    WebGpu,
    Cpu,
    Off,
}

impl Backend {
    pub fn from_setting(value: Option<&str>) -> Self {
        match value.map(str::trim).unwrap_or("") {
            "rocm" => Self::Rocm,
            "webgpu" => Self::WebGpu,
            "cpu" => Self::Cpu,
            "off" => Self::Off,
            _ => Self::Automatic,
        }
    }

    pub fn setting_value(self) -> &'static str {
        match self {
            Self::Automatic => "auto",
            Self::Rocm => "rocm",
            Self::WebGpu => "webgpu",
            Self::Cpu => "cpu",
            Self::Off => "off",
        }
    }

    pub fn current() -> Self {
        Self::from_setting(kog_audio::settings::load_text(BACKEND_SETTING).as_deref())
    }
}

/// The ONNX Runtime library to load: the preference, the environment
/// (`KOG_ONNXRUNTIME`, or `ORT_DYLIB_PATH` as ort documents it), the build
/// `scripts/onnxruntime-rocm.sh` links into Kog's data folder, then the
/// platform's name for it on the library path.
fn library_path() -> PathBuf {
    if let Some(path) = kog_audio::settings::load_text(LIBRARY_SETTING) {
        return PathBuf::from(path);
    }
    for variable in ["KOG_ONNXRUNTIME", "ORT_DYLIB_PATH"] {
        if let Some(path) = std::env::var_os(variable).filter(|path| !path.is_empty()) {
            return PathBuf::from(path);
        }
    }
    if let Some(linked) = kog_core::db::LibraryDb::open()
        .ok()
        .and_then(|db| db.path())
        .and_then(|db| Some(db.parent()?.join("onnxruntime").join("lib").join("libonnxruntime.so")))
        .filter(|path| cfg!(target_os = "linux") && path.is_file())
    {
        return linked;
    }
    PathBuf::from(if cfg!(windows) {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    })
}

fn model_path() -> Option<PathBuf> {
    let db = kog_core::db::LibraryDb::open().ok()?.path()?;
    Some(db.parent()?.join("models").join(MODEL_FILE))
}

/// The model file, downloaded once. Blocking.
fn ensure_model() -> Result<PathBuf, String> {
    let path = model_path().ok_or("Kog's data folder is not available")?;
    if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() >= MODEL_MIN_BYTES) {
        return Ok(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let partial = path.with_extension("onnx.part");
    let response = ureq::get(MODEL_URL)
        .call()
        .map_err(|error| format!("downloading the sound model: {error}"))?;
    let mut reader = response.into_body().into_reader();
    let mut file = std::fs::File::create(&partial).map_err(|error| error.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|error| format!("downloading the sound model: {error}"))?;
    drop(file);
    if std::fs::metadata(&partial).map_or(0, |metadata| metadata.len()) < MODEL_MIN_BYTES {
        let _ = std::fs::remove_file(&partial);
        return Err("The sound model download was incomplete".into());
    }
    std::fs::rename(&partial, &path).map_err(|error| error.to_string())?;
    Ok(path)
}

/// A loaded model, ready to embed songs.
pub struct Embedder {
    session: ort::session::Session,
    /// The providers asked for that this ONNX Runtime has, best first; the
    /// CPU when none is.
    pub providers: Vec<&'static str>,
    filters: Vec<[f32; MELS]>,
    window: Vec<f32>,
    fft: Fft,
}

/// ONNX Runtime allows one environment per process; it is made once it
/// loads, and tried again later while it does not (the library may be
/// installed while Kog runs).
static ENVIRONMENT: std::sync::OnceLock<ort::environment::Environment> = std::sync::OnceLock::new();

fn environment() -> Result<&'static ort::environment::Environment, String> {
    if let Some(environment) = ENVIRONMENT.get() {
        return Ok(environment);
    }
    let library = library_path();
    let environment = ort::init_from(&library)
        .map_err(|error| format!("ONNX Runtime ({}) could not be loaded: {error}", library.display()))?
        .with_name("kog")
        .build()
        .map_err(|error| error.to_string())?;
    Ok(ENVIRONMENT.get_or_init(|| environment))
}

impl Embedder {
    /// Load ONNX Runtime and the model on `backend`. Blocking (it may
    /// download the model first).
    pub fn load(backend: Backend) -> Result<Self, String> {
        use ort::ep::{self, ExecutionProvider};
        if backend == Backend::Off {
            return Err("Sound embeddings are turned off".into());
        }
        let environment = environment()?;
        let model = ensure_model()?;
        let mut providers: Vec<(&'static str, ort::ep::ExecutionProviderDispatch, bool)> = Vec::new();
        let rocm = matches!(backend, Backend::Automatic | Backend::Rocm);
        let other_gpus = backend == Backend::Automatic;
        let webgpu = matches!(backend, Backend::Automatic | Backend::WebGpu);
        if rocm {
            providers.push(("MIGraphX", ep::MIGraphX::default().build(), <ep::MIGraphX as ExecutionProvider>::is_available().unwrap_or(false)));
            providers.push(("ROCm", ep::ROCm::default().build(), <ep::ROCm as ExecutionProvider>::is_available().unwrap_or(false)));
        }
        if other_gpus {
            providers.push(("CUDA", ep::CUDA::default().build(), <ep::CUDA as ExecutionProvider>::is_available().unwrap_or(false)));
            #[cfg(windows)]
            providers.push(("DirectML", ep::DirectML::default().build(), <ep::DirectML as ExecutionProvider>::is_available().unwrap_or(false)));
            #[cfg(target_os = "macos")]
            providers.push(("CoreML", ep::CoreML::default().build(), <ep::CoreML as ExecutionProvider>::is_available().unwrap_or(false)));
        }
        if webgpu {
            providers.push(("WebGPU", ep::WebGPU::default().build(), <ep::WebGPU as ExecutionProvider>::is_available().unwrap_or(false)));
        }
        let available: Vec<&'static str> =
            providers.iter().filter(|(_, _, available)| *available).map(|(name, ..)| *name).collect();
        if matches!(backend, Backend::Rocm | Backend::WebGpu) && available.is_empty() {
            return Err(format!(
                "This ONNX Runtime has no {} support",
                if backend == Backend::Rocm { "ROCm" } else { "WebGPU" }
            ));
        }
        let dispatch: Vec<_> = providers
            .into_iter()
            .filter(|(_, _, available)| *available)
            .map(|(_, dispatch, _)| dispatch)
            .collect();
        let session = ort::session::Session::builder(environment)
            .map_err(|error| error.to_string())?
            .with_execution_providers(dispatch)
            .map_err(|error| error.to_string())?
            .commit_from_file(&model)
            .map_err(|error| format!("loading the sound model: {error}"))?;
        Ok(Self {
            session,
            providers: if available.is_empty() { vec!["CPU"] } else { available },
            filters: slaney_filters(),
            window: hann_window(),
            fft: Fft::new(FFT),
        })
    }

    /// The embedding of ten seconds of a song: from a quarter of the way
    /// in (past most intros), or from the start of a short song. Unit
    /// length, so the dot product of two is their cosine similarity.
    pub fn embed(&mut self, source: PlaybackSource, settings: &DecoderSettings) -> Result<Vec<f32>, String> {
        let clip = read_clip(source, settings)?;
        self.embed_clip(&clip)
    }

    /// The embedding of a clip from [`read_clip`].
    pub fn embed_clip(&mut self, clip: &[f32]) -> Result<Vec<f32>, String> {
        let features = self.log_mel(clip);
        let input = ort::value::Tensor::from_array(([1usize, 1, FRAMES, MELS], features))
            .map_err(|error| error.to_string())?;
        let input_name = self
            .session
            .inputs()
            .first()
            .map(|input| input.name().to_owned())
            .unwrap_or_else(|| "input_features".to_owned());
        let output_name = self
            .session
            .outputs()
            .iter()
            .map(|output| output.name().to_owned())
            .find(|name| name == "audio_embeds")
            .or_else(|| self.session.outputs().first().map(|output| output.name().to_owned()))
            .unwrap_or_else(|| "audio_embeds".to_owned());
        let outputs = self
            .session
            .run(ort::inputs![input_name.as_str() => input])
            .map_err(|error| format!("running the sound model: {error}"))?;
        let (_, values) = outputs[output_name.as_str()]
            .try_extract_tensor::<f32>()
            .map_err(|error| error.to_string())?;
        let mut vector: Vec<f32> = values.iter().take(DIMENSIONS).copied().collect();
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if !norm.is_finite() || norm <= 0.0 {
            return Err("The sound model gave no embedding".into());
        }
        for value in &mut vector {
            *value /= norm;
        }
        Ok(vector)
    }

    /// CLAP's input: a log-mel spectrogram (Slaney mel scale, 64 bands from
    /// 50 Hz to 14 kHz, 1024-sample Hann frames every 480 samples, centred
    /// with reflected edges, power in dB), frames by bands.
    fn log_mel(&mut self, clip: &[f32]) -> Vec<f32> {
        let half = FFT / 2;
        let padded: Vec<f32> = (0..clip.len() + FFT)
            .map(|index| {
                let position = index as isize - half as isize;
                let reflected = if position < 0 {
                    -position
                } else if position as usize >= clip.len() {
                    2 * (clip.len() as isize - 1) - position
                } else {
                    position
                };
                clip[reflected.clamp(0, clip.len() as isize - 1) as usize]
            })
            .collect();
        let mut features = Vec::with_capacity(FRAMES * MELS);
        let mut real = vec![0.0f32; FFT];
        let mut imaginary = vec![0.0f32; FFT];
        for frame in 0..FRAMES {
            let start = frame * HOP;
            for index in 0..FFT {
                real[index] = padded[start + index] * self.window[index];
                imaginary[index] = 0.0;
            }
            self.fft.run(&mut real, &mut imaginary);
            let mut mel = [0.0f32; MELS];
            for (bin, filter) in self.filters.iter().enumerate() {
                let power = real[bin] * real[bin] + imaginary[bin] * imaginary[bin];
                if power == 0.0 {
                    continue;
                }
                for (band, weight) in filter.iter().enumerate() {
                    mel[band] += weight * power;
                }
            }
            for value in mel {
                features.push(10.0 * value.max(1e-10).log10());
            }
        }
        features
    }
}

/// Ten seconds of a song, mono at 48 kHz: repeated to fill ten seconds when
/// it is shorter, as CLAP was trained.
pub fn read_clip(source: PlaybackSource, settings: &DecoderSettings) -> Result<Vec<f32>, String> {
    let mut pcm = PcmReader::open(source, settings.for_recording())?;
    let channels = usize::from(pcm.channels()).max(1);
    let rate = pcm.sample_rate() as usize;
    if rate != SAMPLE_RATE {
        return Err(format!("unexpected sample rate {rate}"));
    }
    let length = pcm.duration().map(|duration| duration.as_secs_f64()).unwrap_or(120.0);
    let start = if length > 12.0 { ((length * 0.25).min(length - 10.0) * rate as f64) as usize } else { 0 };
    let wanted = start + CLIP_SAMPLES;
    let mut mono = Vec::with_capacity(CLIP_SAMPLES);
    let mut bytes = vec![0u8; 64 * 1024];
    let mut leftover = Vec::new();
    let mut frame_index = 0usize;
    'reading: loop {
        let count = pcm.read(&mut bytes).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        leftover.extend_from_slice(&bytes[..count]);
        let frame_bytes = 4 * channels;
        let whole = leftover.len() / frame_bytes * frame_bytes;
        for frame in leftover[..whole].chunks_exact(frame_bytes) {
            if frame_index >= start {
                let mut sum = 0.0f32;
                for sample in frame.chunks_exact(4) {
                    sum += f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
                }
                mono.push(sum / channels as f32);
            }
            frame_index += 1;
            if frame_index >= wanted {
                break 'reading;
            }
        }
        leftover.drain(..whole);
    }
    if mono.len() < SAMPLE_RATE {
        return Err("The song is too short to embed".into());
    }
    let mut clip = Vec::with_capacity(CLIP_SAMPLES);
    while clip.len() < CLIP_SAMPLES {
        let take = (CLIP_SAMPLES - clip.len()).min(mono.len());
        clip.extend_from_slice(&mono[..take]);
    }
    Ok(clip)
}

fn hann_window() -> Vec<f32> {
    // Periodic, as numpy's hanning(n + 1)[:-1].
    (0..FFT)
        .map(|index| (0.5 - 0.5 * (2.0 * std::f64::consts::PI * index as f64 / FFT as f64).cos()) as f32)
        .collect()
}

fn hz_to_slaney(hz: f64) -> f64 {
    let min_log_hz = 1000.0;
    let min_log_mel = 15.0;
    let step = 27.0 / 6.4f64.ln();
    if hz >= min_log_hz { min_log_mel + (hz / min_log_hz).ln() * step } else { 3.0 * hz / 200.0 }
}

fn slaney_to_hz(mel: f64) -> f64 {
    let min_log_hz = 1000.0;
    let min_log_mel = 15.0;
    let step = 6.4f64.ln() / 27.0;
    if mel >= min_log_mel { min_log_hz * (step * (mel - min_log_mel)).exp() } else { 200.0 * mel / 3.0 }
}

/// Slaney-normalised triangular mel filters, one row per FFT bin, as
/// librosa makes them.
fn slaney_filters() -> Vec<[f32; MELS]> {
    let bins = FFT / 2 + 1;
    let low = hz_to_slaney(MIN_HZ);
    let high = hz_to_slaney(MAX_HZ);
    let centres: Vec<f64> =
        (0..MELS + 2).map(|index| slaney_to_hz(low + (high - low) * index as f64 / (MELS + 1) as f64)).collect();
    let nyquist = (SAMPLE_RATE / 2) as f64;
    (0..bins)
        .map(|bin| {
            let hz = nyquist * bin as f64 / (bins - 1) as f64;
            let mut row = [0.0f32; MELS];
            for band in 0..MELS {
                let down = (hz - centres[band]) / (centres[band + 1] - centres[band]);
                let up = (centres[band + 2] - hz) / (centres[band + 2] - centres[band + 1]);
                let weight = down.min(up).max(0.0);
                let norm = 2.0 / (centres[band + 2] - centres[band]);
                row[band] = (weight * norm) as f32;
            }
            row
        })
        .collect()
}

/// An in-place radix-2 FFT of one size.
struct Fft {
    size: usize,
    cos: Vec<f32>,
    sin: Vec<f32>,
}

impl Fft {
    fn new(size: usize) -> Self {
        let cos = (0..size / 2).map(|k| (2.0 * std::f64::consts::PI * k as f64 / size as f64).cos() as f32).collect();
        let sin = (0..size / 2).map(|k| (2.0 * std::f64::consts::PI * k as f64 / size as f64).sin() as f32).collect();
        Self { size, cos, sin }
    }

    fn run(&self, real: &mut [f32], imaginary: &mut [f32]) {
        let n = self.size;
        let mut j = 0usize;
        for i in 1..n {
            let mut bit = n >> 1;
            while j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j |= bit;
            if i < j {
                real.swap(i, j);
                imaginary.swap(i, j);
            }
        }
        let mut length = 2;
        while length <= n {
            let step = n / length;
            for start in (0..n).step_by(length) {
                for k in 0..length / 2 {
                    let (wr, wi) = (self.cos[k * step], -self.sin[k * step]);
                    let a = start + k;
                    let b = a + length / 2;
                    let tr = real[b] * wr - imaginary[b] * wi;
                    let ti = real[b] * wi + imaginary[b] * wr;
                    real[b] = real[a] - tr;
                    imaginary[b] = imaginary[a] - ti;
                    real[a] += tr;
                    imaginary[a] += ti;
                }
            }
            length <<= 1;
        }
    }
}

/// How far apart two embeddings are: 0 for the same direction, 2 for
/// opposite.
pub fn distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 2.0;
    }
    1.0 - a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>()
}

/// The average direction of several embeddings.
pub fn centroid<'a>(vectors: impl IntoIterator<Item = &'a [f32]>) -> Option<Vec<f32>> {
    let mut sum = vec![0.0f32; DIMENSIONS];
    let mut count = 0;
    for vector in vectors {
        if vector.len() != DIMENSIONS {
            continue;
        }
        for (total, value) in sum.iter_mut().zip(vector) {
            *total += value;
        }
        count += 1;
    }
    let norm = sum.iter().map(|value| value * value).sum::<f32>().sqrt();
    (count > 0 && norm > 0.0).then(|| sum.into_iter().map(|value| value / norm).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fft_finds_a_pure_tone() {
        let fft = Fft::new(64);
        let mut real: Vec<f32> = (0..64).map(|i| (2.0 * std::f32::consts::PI * 5.0 * i as f32 / 64.0).cos()).collect();
        let mut imaginary = vec![0.0; 64];
        fft.run(&mut real, &mut imaginary);
        let peak = (0..32)
            .max_by(|a, b| (real[*a].hypot(imaginary[*a])).total_cmp(&real[*b].hypot(imaginary[*b])))
            .unwrap();
        assert_eq!(peak, 5);
    }

    #[test]
    fn slaney_filters_cover_the_band() {
        let filters = slaney_filters();
        assert_eq!(filters.len(), FFT / 2 + 1);
        for band in 0..MELS {
            assert!(filters.iter().any(|row| row[band] > 0.0), "band {band} is empty");
        }
    }
}
