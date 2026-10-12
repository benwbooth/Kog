//! The playing row's level meter, measured the way iOS measures the Now
//! Playing waveform in the Dynamic Island (MediaControls' MRUAudioAnalyzer
//! and MRUWaveformData, read from the iOS 26.5 runtime): twenty times a
//! second, a Hann-windowed FFT of the latest samples is summed into six
//! bands, compressed, tilted down toward the treble and shaped by a fixed
//! curve into bar heights from 0 to 1.

/// Bars drawn, lowest band on the left.
pub const BARS: usize = 6;
/// How often the bars get new heights.
pub const UPDATES_PER_SECOND: u32 = 20;
/// How long a bar takes to spring to a new height, in milliseconds (iOS
/// takes 325; shorter reads as bouncier).
pub const SPRING_MS: u32 = 140;
/// How much wider a bar at full height is than a quiet one.
pub const WIDTH_GROWTH: f32 = 0.66;

const STOPS_HZ: [f32; BARS + 1] = [20.0, 81.0, 170.0, 527.0, 1_500.0, 4_500.0, 10_000.0];
const AMPLITUDE_GAIN: f32 = 0.6;
const EXPONENTIAL_GAIN: f32 = 0.4;
const FREQUENCY_EXPONENT: f32 = -0.15;
const ADJUSTMENT: [f64; 5] = [0.0, 3.840_823, -8.182_433, 7.772_333, -2.430_722];

/// The FFT length for a sample rate: the power of two nearest half an
/// update's worth of samples (1024 at 44.1 or 48 kHz).
pub fn frame_len(sample_rate: u32) -> usize {
    let half_update = (sample_rate as f32 / UPDATES_PER_SECOND as f32).ceil() * 0.5;
    1 << (half_update.max(2.0).log2().round() as u32)
}

/// Bar heights for the latest `frame_len(sample_rate)` mono samples.
pub fn levels(samples: &[f32], sample_rate: u32) -> [f32; BARS] {
    let n = samples.len().next_power_of_two().max(4);
    let mut real = vec![0.0_f32; n];
    let mut imag = vec![0.0_f32; n];
    // vDSP_hann_window with vDSP_HANN_NORM.
    for (i, sample) in samples.iter().enumerate() {
        let phase = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
        real[i] = sample * 0.8165 * (1.0 - phase.cos());
    }
    fft(&mut real, &mut imag);
    let half = n / 2;
    // vDSP's packed real FFT doubles every bin and puts Nyquist beside DC;
    // its magnitudes are then scaled by 2 / half.
    let scale = 2.0 / half as f32;
    let magnitude = |i: usize| {
        let (re, im) = if i == 0 { (real[0], real[half]) } else { (real[i], imag[i]) };
        2.0 * (re * re + im * im).sqrt() * scale
    };

    let mut bands = [0.0_f32; BARS];
    let bin_hz = sample_rate as f32 / (2 * half) as f32;
    let mut band = 0;
    for i in 0..half {
        if band == STOPS_HZ.len() - 1 {
            break;
        }
        let frequency = i as f32 * bin_hz;
        let (low, high) = (STOPS_HZ[band], STOPS_HZ[band + 1]);
        if frequency >= low && frequency <= high {
            bands[band] += magnitude(i);
            continue;
        }
        // A bin past the band moves on to the band it falls in, and (as
        // iOS does) counts there divided by the old band's width.
        if STOPS_HZ.len() - 2 > band && frequency >= high {
            loop {
                band += 1;
                if STOPS_HZ.len() - 2 <= band || frequency < STOPS_HZ[band + 1] {
                    break;
                }
            }
        }
        if band < BARS {
            bands[band] += magnitude(i) / (high - low);
        }
    }

    std::array::from_fn(|index| {
        let compressed = (bands[index] * AMPLITUDE_GAIN).powf(EXPONENTIAL_GAIN);
        let x = f64::from((1.0 + index as f32 * FREQUENCY_EXPONENT) * compressed);
        let curve = ADJUSTMENT
            .iter()
            .enumerate()
            .map(|(power, c)| c * x.powi(power as i32))
            .sum::<f64>();
        (compressed * curve as f32).clamp(0.0, 1.0)
    })
}

/// In-place radix-2 FFT; `real.len()` must be a power of two.
fn fft(real: &mut [f32], imag: &mut [f32]) {
    let n = real.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            real.swap(i, j);
            imag.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (sin, cos) = (angle * k as f64).sin_cos();
                let (wr, wi) = (cos as f32, sin as f32);
                let (a, b) = (start + k, start + k + len / 2);
                let tr = real[b] * wr - imag[b] * wi;
                let ti = real[b] * wi + imag[b] * wr;
                real[b] = real[a] - tr;
                imag[b] = imag[a] - ti;
                real[a] += tr;
                imag[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// Beat emphasis on top of the iOS heights: each bar rides on how far its
/// band is above its own recent average, so beats kick it up and lulls drop
/// it instead of it hovering. Silence stays dark.
#[derive(Clone, Debug, Default)]
pub struct Bounce {
    average: [f32; BARS],
}

impl Bounce {
    pub fn apply(&mut self, levels: [f32; BARS]) -> [f32; BARS] {
        std::array::from_fn(|index| {
            let level = levels[index];
            self.average[index] += 0.15 * (level - self.average[index]);
            // The treble moves less in the iOS heights, so it gets more kick.
            let kick = 2.5 * (1.0 + 0.35 * index as f32);
            let gate = (level / 0.03).clamp(0.0, 1.0);
            ((0.35 + kick * (level - self.average[index])) * gate).clamp(0.0, 1.0)
        })
    }

    pub fn reset(&mut self) {
        self.average = [0.0; BARS];
    }
}

/// Feeds a running stream into the analysis: keeps the latest frame and
/// produces new heights `UPDATES_PER_SECOND` times a second.
#[derive(Clone, Debug, Default)]
pub struct Analyzer {
    sample_rate: u32,
    frame: Vec<f32>,
    cursor: usize,
    since_update: u32,
    levels: [f32; BARS],
    bounce: Bounce,
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            frame: vec![0.0; frame_len(sample_rate)],
            ..Self::default()
        }
    }

    /// Add one mono sample; true when the heights were just updated.
    pub fn push(&mut self, sample: f32) -> bool {
        if self.frame.is_empty() {
            return false;
        }
        self.frame[self.cursor] = sample;
        self.cursor = (self.cursor + 1) % self.frame.len();
        self.since_update += 1;
        if self.since_update < self.sample_rate / UPDATES_PER_SECOND {
            return false;
        }
        self.since_update = 0;
        let ordered: Vec<f32> = self.frame[self.cursor..]
            .iter()
            .chain(&self.frame[..self.cursor])
            .copied()
            .collect();
        self.levels = self.bounce.apply(levels(&ordered, self.sample_rate));
        true
    }

    pub fn levels(&self) -> [f32; BARS] {
        self.levels
    }

    pub fn reset(&mut self) {
        self.frame.fill(0.0);
        self.cursor = 0;
        self.since_update = 0;
        self.levels = [0.0; BARS];
        self.bounce.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frequency: f32, amplitude: f32) -> Vec<f32> {
        (0..1024)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * frequency * i as f32 / 48_000.0).sin())
            .collect()
    }

    #[test]
    fn frame_is_1024_at_common_rates() {
        assert_eq!(frame_len(48_000), 1024);
        assert_eq!(frame_len(44_100), 1024);
    }

    #[test]
    fn tones_light_their_own_band_and_silence_is_dark() {
        assert_eq!(levels(&[0.0; 1024], 48_000), [0.0; BARS]);
        let bass = levels(&sine(120.0, 0.5), 48_000);
        let treble = levels(&sine(6_000.0, 0.5), 48_000);
        assert!(bass[1] > 0.5 && bass[5] < bass[1], "{bass:?}");
        assert!(treble[5] > 0.3 && treble[0] < treble[5], "{treble:?}");
    }
}
