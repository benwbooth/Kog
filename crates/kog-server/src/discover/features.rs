//! Sound features for similarity: what a song is like, measured by playing
//! the opening of it once through its own decoder, without output.
//!
//! Chip formats report their channels' notes and instruments to the channel
//! inspector as they play; those frames give the note density, the pitch
//! range and key, how many voices sound at once and how much of the song is
//! tone, noise or samples. Every format, chip or not, also gives its PCM:
//! loudness, dynamics, the six-band spectrum the level meter uses, and an
//! onset envelope whose periodicity estimates the tempo.
//!
//! The result is a short fixed vector (see the `F_*` indices) compared by
//! [`distance`]. Analysis is cheap next to an audio model (tens of
//! milliseconds of emulation per second of music) and runs on the
//! background indexer at idle priority.

use std::io::Read;
use std::time::Duration;

use kog_audio::decoder::{DecoderSettings, PlaybackSource};
use kog_audio::inspection::TimedFrame;
use kog_audio::streaming::PcmReader;

/// How much of each song is played for analysis.
pub const ANALYSIS_SECONDS: f64 = 45.0;
/// Songs shorter than this are measured whole; the analysis needs at least
/// a few seconds to say anything.
const MIN_SECONDS: f64 = 3.0;

/// Which way of measuring made a vector; vectors from another version are
/// measured again.
pub const VERSION: f32 = 1.0;

pub const F_VERSION: usize = 0;
pub const F_TEMPO: usize = 1;
pub const F_PULSE: usize = 2;
pub const F_NOTE_RATE: usize = 3;
pub const F_VOICES: usize = 4;
pub const F_PITCH: usize = 5;
pub const F_PITCH_SPREAD: usize = 6;
pub const F_TONAL: usize = 7;
pub const F_NOISE: usize = 8;
pub const F_SAMPLE: usize = 9;
pub const F_INSTRUMENTS: usize = 10;
pub const F_MAJOR: usize = 11;
pub const F_KEY_CLARITY: usize = 12;
pub const F_LOUDNESS: usize = 13;
pub const F_DYNAMICS: usize = 14;
pub const F_BANDS: usize = 15;
pub const F_FLUX: usize = 21;
pub const F_HAS_NOTES: usize = 22;
pub const F_CHROMA: usize = 23;
pub const F_KEY: usize = 35;
pub const LEN: usize = 36;

/// Envelope rate for onsets and tempo, in frames per second.
const ENVELOPE_RATE: usize = 100;

/// Krumhansl–Kessler key profiles, starting at the tonic.
const MAJOR_PROFILE: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR_PROFILE: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

/// Measure a song. Blocking, and as slow as its decoder: run it on a
/// background thread with a timeout.
pub fn analyze(source: PlaybackSource, settings: &DecoderSettings) -> Result<Vec<f32>, String> {
    let mut pcm = PcmReader::open(source, settings.for_recording())?;
    let limit = pcm
        .duration()
        .map_or(ANALYSIS_SECONDS, |duration| duration.as_secs_f64().min(ANALYSIS_SECONDS));
    measure(&mut pcm, limit)
}

fn measure(pcm: &mut PcmReader, limit: f64) -> Result<Vec<f32>, String> {
    let monitor = pcm.inspection_monitor();
    let rate = pcm.sample_rate() as usize;
    let channels = usize::from(pcm.channels()).max(1);
    let window = kog_audio::playback_order::waveform::frame_len(rate as u32);
    let band_hop = rate / kog_audio::playback_order::waveform::UPDATES_PER_SECOND as usize;
    let envelope_hop = rate / ENVELOPE_RATE;

    let mut notes = NoteStats::default();
    let mut after: Option<f64> = None;
    let mut drained = 0.0;

    let mut bytes = vec![0u8; 64 * 1024];
    let mut leftover: Vec<u8> = Vec::new();
    let mut mono: Vec<f32> = Vec::with_capacity(rate);
    let mut consumed_frames = 0usize;

    let mut band_sums = [0.0f64; 6];
    let mut band_count = 0usize;
    let mut previous_bands = [0.0f32; 6];
    let mut flux_sum = 0.0f64;
    let mut envelope: Vec<f32> = Vec::new();
    let mut energy_window = 0.0f64;
    let mut energy_count = 0usize;
    let mut previous_log_energy = 0.0f32;
    let mut second_energy = 0.0f64;
    let mut second_count = 0usize;
    let mut second_levels: Vec<f32> = Vec::new();
    let mut total_energy = 0.0f64;
    let mut total_count = 0usize;

    loop {
        let count = pcm.read(&mut bytes).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        leftover.extend_from_slice(&bytes[..count]);
        let frame_bytes = 4 * channels;
        let whole = leftover.len() / frame_bytes * frame_bytes;
        for frame in leftover[..whole].chunks_exact(frame_bytes) {
            let mut sum = 0.0f32;
            for sample in frame.chunks_exact(4) {
                sum += f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
            }
            let value = sum / channels as f32;
            mono.push(value);
            consumed_frames += 1;
            let energy = f64::from(value * value);
            energy_window += energy;
            energy_count += 1;
            second_energy += energy;
            second_count += 1;
            total_energy += energy;
            total_count += 1;
            if energy_count == envelope_hop {
                let log_energy = (10.0 * ((energy_window / energy_count as f64) + 1e-10).log10()) as f32;
                envelope.push((log_energy - previous_log_energy).max(0.0));
                previous_log_energy = log_energy;
                energy_window = 0.0;
                energy_count = 0;
            }
            if second_count == rate {
                second_levels.push((10.0 * ((second_energy / rate as f64) + 1e-10).log10()) as f32);
                second_energy = 0.0;
                second_count = 0;
            }
            if consumed_frames % band_hop == 0 && mono.len() >= window {
                let levels =
                    kog_audio::playback_order::waveform::levels(&mono[mono.len() - window..], rate as u32);
                for (index, level) in levels.iter().enumerate() {
                    band_sums[index] += f64::from(*level);
                    flux_sum += f64::from((level - previous_bands[index]).max(0.0));
                }
                previous_bands = levels;
                band_count += 1;
                if mono.len() > window * 4 {
                    mono.drain(..mono.len() - window);
                }
            }
        }
        leftover.drain(..whole);
        let seconds = consumed_frames as f64 / rate as f64;
        if seconds - drained >= 0.5 {
            drain_frames(&monitor, &mut after, seconds, &mut notes);
            drained = seconds;
        }
        if seconds >= limit {
            break;
        }
    }
    let seconds = consumed_frames as f64 / rate as f64;
    drain_frames(&monitor, &mut after, seconds + 1.0, &mut notes);
    if seconds < MIN_SECONDS.min(limit * 0.9) {
        return Err("The song is too short to measure".into());
    }
    let loudness = (10.0 * ((total_energy / total_count.max(1) as f64) + 1e-10).log10()) as f32;
    if loudness < -70.0 {
        return Err("The song is silent".into());
    }

    let mut vector = vec![0.0f32; LEN];
    vector[F_VERSION] = VERSION;
    // Note onsets are sharper than the PCM's energy steps when the decoder
    // reports them; otherwise the energy envelope stands alone.
    let onset_envelope = if notes.onset_times.len() >= 16 {
        let mut train = vec![0.0f32; (seconds * ENVELOPE_RATE as f64) as usize + 1];
        for time in &notes.onset_times {
            if let Some(slot) = train.get_mut((time * ENVELOPE_RATE as f64) as usize) {
                *slot += 1.0;
            }
        }
        for (slot, energy) in train.iter_mut().zip(&envelope) {
            *slot += energy * 0.05;
        }
        train
    } else {
        envelope.clone()
    };
    let (tempo, pulse) = tempo(&onset_envelope);
    vector[F_TEMPO] = tempo;
    vector[F_PULSE] = pulse;
    vector[F_LOUDNESS] = loudness;
    vector[F_DYNAMICS] = standard_deviation(&second_levels);
    for (index, sum) in band_sums.iter().enumerate() {
        vector[F_BANDS + index] = (sum / band_count.max(1) as f64) as f32;
    }
    vector[F_FLUX] = (flux_sum / band_count.max(1) as f64) as f32;
    if notes.time > 0.0 && notes.onsets > 0 {
        let time = notes.time as f32;
        vector[F_HAS_NOTES] = 1.0;
        vector[F_NOTE_RATE] = notes.onsets as f32 / seconds as f32;
        vector[F_VOICES] = (notes.voice_time / notes.time) as f32;
        let weight = notes.pitch_weight.max(1e-6);
        let mean = notes.pitch_sum / weight;
        vector[F_PITCH] = mean as f32;
        vector[F_PITCH_SPREAD] = ((notes.pitch_squares / weight) - mean * mean).max(0.0).sqrt() as f32;
        let kinds = (notes.tonal_time + notes.noise_time + notes.sample_time).max(1e-6);
        vector[F_TONAL] = (notes.tonal_time / kinds) as f32;
        vector[F_NOISE] = (notes.noise_time / kinds) as f32;
        vector[F_SAMPLE] = (notes.sample_time / kinds) as f32;
        vector[F_INSTRUMENTS] = notes.instruments.len() as f32;
        let total: f32 = notes.chroma.iter().sum::<f64>() as f32;
        if total > 0.0 {
            let chroma: Vec<f32> = notes.chroma.iter().map(|value| *value as f32 / total).collect();
            vector[F_CHROMA..F_CHROMA + 12].copy_from_slice(&chroma);
            let (key, major, clarity) = estimate_key(&chroma);
            vector[F_KEY] = key as f32;
            vector[F_MAJOR] = if major { 1.0 } else { 0.0 };
            vector[F_KEY_CLARITY] = clarity;
        }
        let _ = time;
    }
    Ok(vector)
}

#[derive(Default)]
struct NoteStats {
    previous: Option<(f64, Vec<(u32, Vec<i32>, bool, String)>)>,
    time: f64,
    onsets: usize,
    onset_times: Vec<f64>,
    voice_time: f64,
    pitch_sum: f64,
    pitch_squares: f64,
    pitch_weight: f64,
    tonal_time: f64,
    noise_time: f64,
    sample_time: f64,
    chroma: [f64; 12],
    instruments: std::collections::HashSet<String>,
}

impl NoteStats {
    fn push(&mut self, frame: TimedFrame) {
        let current: Vec<(u32, Vec<i32>, bool, String)> = frame
            .data
            .channels
            .iter()
            .map(|channel| {
                let keys: Vec<i32> = if channel.active {
                    channel.notes.iter().map(|note| note.key.round() as i32).collect()
                } else {
                    Vec::new()
                };
                (channel.id, keys, channel.active, channel.kind.clone())
            })
            .collect();
        for channel in &frame.data.channels {
            if channel.active && !channel.instrument.trim().is_empty() {
                self.instruments
                    .insert(format!("{}:{}", channel.id, channel.instrument.trim()));
            }
        }
        if let Some((time, previous)) = self.previous.take() {
            let span = (frame.time - time).clamp(0.0, 0.5);
            self.time += span;
            let mut voices = 0usize;
            for (id, keys, active, kind) in &previous {
                if !*active {
                    continue;
                }
                match kind.as_str() {
                    "noise" => self.noise_time += span,
                    "sample" | "percussion" => self.sample_time += span,
                    _ => self.tonal_time += span,
                }
                if keys.is_empty() {
                    if kind != "tonal" {
                        voices += 1;
                    }
                    continue;
                }
                voices += 1;
                for key in keys {
                    let weight = span;
                    self.pitch_sum += f64::from(*key) * weight;
                    self.pitch_squares += f64::from(*key) * f64::from(*key) * weight;
                    self.pitch_weight += weight;
                    self.chroma[key.rem_euclid(12) as usize] += weight;
                }
                let _ = id;
            }
            self.voice_time += voices as f64 * span;
            for (id, keys, active, kind) in &current {
                let before = previous.iter().find(|(other, ..)| other == id);
                let was_active = before.is_some_and(|(_, _, active, _)| *active);
                let new_note = keys
                    .iter()
                    .any(|key| !before.is_some_and(|(_, keys, ..)| keys.contains(key)));
                let struck = (*active && !was_active) && (kind != "tonal" || !keys.is_empty());
                if new_note || struck {
                    self.onsets += 1;
                    self.onset_times.push(frame.time);
                }
            }
        }
        self.previous = Some((frame.time, current));
    }
}

fn drain_frames(
    monitor: &kog_audio::inspection::Monitor,
    after: &mut Option<f64>,
    through: f64,
    notes: &mut NoteStats,
) {
    let batch = monitor
        .take_frames()
        .unwrap_or_else(|| monitor.recording_frames(*after, Duration::from_secs_f64(through)));
    if let Some(last) = batch.last() {
        *after = Some(last.time);
    }
    for frame in batch {
        notes.push(frame);
    }
}

/// The tempo in beats per minute (60–200) with how clearly it pulses (0–1),
/// from the autocorrelation of an onset envelope at [`ENVELOPE_RATE`].
fn tempo(envelope: &[f32]) -> (f32, f32) {
    if envelope.len() < ENVELOPE_RATE * 4 {
        return (0.0, 0.0);
    }
    let mean = envelope.iter().sum::<f32>() / envelope.len() as f32;
    let centered: Vec<f32> = envelope.iter().map(|value| value - mean).collect();
    let energy: f32 = centered.iter().map(|value| value * value).sum();
    if energy <= 1e-9 {
        return (0.0, 0.0);
    }
    let min_lag = ENVELOPE_RATE * 60 / 200;
    let max_lag = ENVELOPE_RATE * 60 / 60;
    let mut best = (0usize, 0.0f32);
    for lag in min_lag..=max_lag.min(centered.len() / 2) {
        let score: f32 = centered.iter().zip(&centered[lag..]).map(|(a, b)| a * b).sum::<f32>()
            / energy;
        // A gentle preference for moderate tempos keeps half and double
        // tempo guesses apart from the felt beat.
        let bpm = 60.0 * ENVELOPE_RATE as f32 / lag as f32;
        let weighted = score * (1.0 - 0.25 * ((bpm / 120.0).log2()).abs());
        if weighted > best.1 {
            best = (lag, weighted);
        }
    }
    if best.0 == 0 {
        return (0.0, 0.0);
    }
    (60.0 * ENVELOPE_RATE as f32 / best.0 as f32, best.1.clamp(0.0, 1.0))
}

fn standard_deviation(values: &[f32]) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    (values.iter().map(|value| (value - mean).powi(2)).sum::<f32>() / values.len() as f32).sqrt()
}

/// The best fitting key (pitch class of the tonic), whether it is major,
/// and how clearly it wins (correlation margin over the runner-up).
fn estimate_key(chroma: &[f32]) -> (usize, bool, f32) {
    let correlate = |profile: &[f32; 12], tonic: usize| {
        let profile_mean = profile.iter().sum::<f32>() / 12.0;
        let chroma_mean = chroma.iter().sum::<f32>() / 12.0;
        let mut numerator = 0.0;
        let mut a = 0.0;
        let mut b = 0.0;
        for index in 0..12 {
            let x = chroma[(tonic + index) % 12] - chroma_mean;
            let y = profile[index] - profile_mean;
            numerator += x * y;
            a += x * x;
            b += y * y;
        }
        if a <= 0.0 || b <= 0.0 { 0.0 } else { numerator / (a * b).sqrt() }
    };
    let mut scores: Vec<(f32, usize, bool)> = (0..12)
        .flat_map(|tonic| {
            [
                (correlate(&MAJOR_PROFILE, tonic), tonic, true),
                (correlate(&MINOR_PROFILE, tonic), tonic, false),
            ]
        })
        .collect();
    scores.sort_by(|a, b| b.0.total_cmp(&a.0));
    let clarity = (scores[0].0 - scores[1].0).clamp(0.0, 1.0);
    (scores[0].1, scores[0].2, clarity)
}

/// How different two songs sound: 0 for the same, about 1 for unrelated,
/// larger for opposites. Note-based measures only count when both songs
/// have them.
pub fn distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != LEN || b.len() != LEN {
        return 2.0;
    }
    let ratio = |x: f32, y: f32| ((x + 1.0) / (y + 1.0)).ln().abs();
    let mut total = 0.0f32;
    let mut weights = 0.0f32;
    let mut add = |difference: f32, scale: f32, weight: f32| {
        let normalized = (difference / scale).min(3.0);
        total += normalized * normalized * weight;
        weights += weight;
    };
    if a[F_TEMPO] > 0.0 && b[F_TEMPO] > 0.0 {
        // Half and double tempo feel alike.
        let octaves = (a[F_TEMPO] / b[F_TEMPO]).log2();
        let folded = (octaves - octaves.round()).abs().min(octaves.abs());
        add(folded, 0.25, 1.5);
    }
    add(a[F_PULSE] - b[F_PULSE], 0.3, 0.5);
    add(a[F_LOUDNESS] - b[F_LOUDNESS], 8.0, 0.7);
    add(a[F_DYNAMICS] - b[F_DYNAMICS], 4.0, 0.5);
    let bands: f32 = (0..6)
        .map(|index| (a[F_BANDS + index] - b[F_BANDS + index]).powi(2))
        .sum::<f32>()
        .sqrt();
    add(bands, 0.2, 2.0);
    add(a[F_FLUX] - b[F_FLUX], 0.15, 0.7);
    if a[F_HAS_NOTES] > 0.0 && b[F_HAS_NOTES] > 0.0 {
        add(ratio(a[F_NOTE_RATE], b[F_NOTE_RATE]), 0.5, 1.5);
        add(a[F_VOICES] - b[F_VOICES], 1.5, 1.0);
        add(a[F_PITCH] - b[F_PITCH], 10.0, 0.8);
        add(a[F_PITCH_SPREAD] - b[F_PITCH_SPREAD], 5.0, 0.6);
        add(a[F_TONAL] - b[F_TONAL], 0.3, 0.6);
        add(a[F_NOISE] - b[F_NOISE], 0.25, 0.6);
        add(a[F_SAMPLE] - b[F_SAMPLE], 0.25, 0.6);
        add(ratio(a[F_INSTRUMENTS], b[F_INSTRUMENTS]), 0.8, 0.5);
        if a[F_KEY_CLARITY] > 0.05 && b[F_KEY_CLARITY] > 0.05 {
            add(a[F_MAJOR] - b[F_MAJOR], 1.0, 0.8);
        }
    } else if (a[F_HAS_NOTES] > 0.0) != (b[F_HAS_NOTES] > 0.0) {
        // A chip tune against a recording: different worlds.
        add(1.0, 1.0, 1.0);
    }
    (total / weights.max(1e-6)).sqrt()
}

/// The average of several vectors, for a listener's taste in one style.
pub fn centroid<'a>(vectors: impl IntoIterator<Item = &'a [f32]>) -> Option<Vec<f32>> {
    let mut sum = vec![0.0f32; LEN];
    let mut notes = 0usize;
    let mut count = 0usize;
    let mut tempo_count = 0usize;
    for vector in vectors {
        if vector.len() != LEN {
            continue;
        }
        count += 1;
        for index in 0..LEN {
            if index == F_TEMPO && vector[index] <= 0.0 {
                continue;
            }
            sum[index] += vector[index];
        }
        if vector[F_TEMPO] > 0.0 {
            tempo_count += 1;
        }
        if vector[F_HAS_NOTES] > 0.0 {
            notes += 1;
        }
    }
    if count == 0 {
        return None;
    }
    for (index, value) in sum.iter_mut().enumerate() {
        *value /= if index == F_TEMPO { tempo_count.max(1) } else { count } as f32;
    }
    sum[F_HAS_NOTES] = if notes * 2 >= count { 1.0 } else { 0.0 };
    sum[F_VERSION] = VERSION;
    Some(sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clear_pulse_gives_its_tempo() {
        // An onset every half second: 120 BPM.
        let mut envelope = vec![0.0f32; ENVELOPE_RATE * 20];
        for index in (0..envelope.len()).step_by(ENVELOPE_RATE / 2) {
            envelope[index] = 1.0;
        }
        let (bpm, pulse) = tempo(&envelope);
        assert!((bpm - 120.0).abs() < 3.0, "{bpm}");
        assert!(pulse > 0.3, "{pulse}");
    }

    #[test]
    fn a_c_major_scale_is_in_c_major() {
        let mut chroma = [0.0f32; 12];
        for pitch in [0, 2, 4, 5, 7, 9, 11] {
            chroma[pitch] = 1.0;
        }
        chroma[0] = 2.0;
        chroma[7] = 1.5;
        let (key, major, _) = estimate_key(&chroma);
        assert_eq!((key, major), (0, true));
    }

    #[test]
    fn songs_are_closest_to_themselves() {
        let mut a = vec![0.0f32; LEN];
        a[F_VERSION] = VERSION;
        a[F_TEMPO] = 140.0;
        a[F_LOUDNESS] = -14.0;
        a[F_HAS_NOTES] = 1.0;
        a[F_NOTE_RATE] = 8.0;
        let mut b = a.clone();
        b[F_TEMPO] = 70.0;
        let mut c = a.clone();
        c[F_NOTE_RATE] = 1.0;
        c[F_LOUDNESS] = -30.0;
        assert_eq!(distance(&a, &a), 0.0);
        assert!(distance(&a, &b) < 0.1, "double time feels alike");
        assert!(distance(&a, &c) > distance(&a, &b));
    }
}
