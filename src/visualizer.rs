//! A bounded, allocation-free audio tap. Analysis happens only on UI requests,
//! never on the audio callback. A snapshot may straddle callback blocks; this
//! is a display meter, not a sample-accurate recording interface.
use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicUsize, Ordering},
};

const SIZE: usize = 2048;

struct Buffer {
    samples: [AtomicU32; SIZE],
    cursor: AtomicUsize,
    rate: AtomicU32,
}

#[derive(Clone)]
pub struct AudioTap(Arc<Buffer>);

impl Default for AudioTap {
    fn default() -> Self {
        Self(Arc::new(Buffer {
            samples: std::array::from_fn(|_| AtomicU32::new(0)),
            cursor: AtomicUsize::new(0),
            rate: AtomicU32::new(48_000),
        }))
    }
}

impl AudioTap {
    pub fn set_rate(&self, rate: u32) {
        self.0.rate.store(rate, Ordering::Relaxed);
    }

    pub fn push(&self, sample: f32) {
        let cursor = self.0.cursor.load(Ordering::Relaxed);
        let sample = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.0.samples[cursor % SIZE].store(sample.to_bits(), Ordering::Relaxed);
        self.0
            .cursor
            .store(cursor.wrapping_add(1), Ordering::Release);
    }

    pub fn reset(&self) {
        for sample in &self.0.samples {
            sample.store(0, Ordering::Relaxed);
        }
    }

    pub fn frame(&self, playing: bool) -> String {
        let cursor = self.0.cursor.load(Ordering::Acquire);
        let samples: [f32; SIZE] = std::array::from_fn(|i| {
            if playing {
                f32::from_bits(
                    self.0.samples[(cursor.wrapping_add(i)) % SIZE].load(Ordering::Relaxed),
                )
            } else {
                0.0
            }
        });
        // A short oscilloscope window retains detail instead of averaging it away.
        let wave = &samples[SIZE - 256..];
        let spectrum = spectrum(&samples, self.0.rate.load(Ordering::Relaxed));
        serde_json::json!({"wave": wave, "spectrum": spectrum}).to_string()
    }
}

fn spectrum(samples: &[f32; SIZE], rate: u32) -> Vec<f32> {
    let mut real = [0.0_f32; SIZE];
    let mut imag = [0.0_f32; SIZE];
    for i in 0..SIZE {
        let reversed = i.reverse_bits() >> (usize::BITS - SIZE.ilog2());
        real[reversed] =
            samples[i] * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (SIZE - 1) as f32).cos());
    }
    let mut length = 2;
    while length <= SIZE {
        for start in (0..SIZE).step_by(length) {
            for k in 0..length / 2 {
                let angle = -std::f32::consts::TAU * k as f32 / length as f32;
                let (sin, cos) = angle.sin_cos();
                let a = start + k;
                let b = a + length / 2;
                let r = cos * real[b] - sin * imag[b];
                let im = sin * real[b] + cos * imag[b];
                real[b] = real[a] - r;
                imag[b] = imag[a] - im;
                real[a] += r;
                imag[a] += im;
            }
        }
        length *= 2;
    }
    let top = (rate as f32 / 2.0).min(20_000.0).max(40.0);
    (0..40)
        .map(|band| {
            let bin = |edge: i32| {
                ((30.0 * (top / 30.0).powf(edge as f32 / 40.0) * SIZE as f32 / rate.max(1) as f32)
                    .round() as usize)
                    .clamp(1, SIZE / 2)
            };
            let low = bin(band);
            let high = bin(band + 1).max(low + 1).min(SIZE / 2 + 1);
            let magnitude = (low..high)
                .map(|i| real[i].hypot(imag[i]) * 4.0 / SIZE as f32)
                .fold(0.0_f32, f32::max);
            ((20.0 * magnitude.max(1e-6).log10() + 72.0) / 72.0).clamp(0.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_and_paused_frames_are_zero() {
        let tap = AudioTap::default();
        for _ in 0..SIZE {
            tap.push(0.75);
        }
        let paused: serde_json::Value = serde_json::from_str(&tap.frame(false)).unwrap();
        assert!(paused["wave"].as_array().unwrap().iter().all(|v| v == 0.0));
        tap.reset();
        assert!(spectrum(&[0.0; SIZE], 48000).iter().all(|v| *v == 0.0));
    }
    #[test]
    fn fft_finds_one_kilohertz() {
        let samples = std::array::from_fn(|i| {
            (std::f32::consts::TAU * 1000.0 * i as f32 / 48000.0).sin() * 0.5
        });
        let bands = spectrum(&samples, 48000);
        let peak = bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert!((20..=22).contains(&peak), "peak band {peak}");
    }
    #[test]
    fn ring_wraps_and_rejects_nonfinite_samples() {
        let tap = AudioTap::default();
        for _ in 0..SIZE * 3 {
            tap.push(0.5);
        }
        tap.push(f32::NAN);
        let frame: serde_json::Value = serde_json::from_str(&tap.frame(true)).unwrap();
        assert_eq!(frame["wave"][254], 0.5);
        assert_eq!(frame["wave"][255], 0.0);
    }
}
