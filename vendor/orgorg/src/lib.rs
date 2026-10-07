#![cfg_attr(not(feature = "fma"), no_std)]
#![cfg_attr(feature = "fma-nightly", feature(float_algebraic))]

//! `no_std` compatible Cave Story Organya Music Player.
//!
//! Partially based on bisqwit's C++ OrgPlay.
//!
//! Designed to be 100% pure, side-effect free.
//!
//! # Example
//! ```no_run
//! // Basic example for playing Org-02 music with original Cave Story drum sound effects.
//! use orgorg::{OrgPlay, OrgPlayBuilder, AssetByRef, interp_impls::Linear};
//!
//! let wavetable = todo!();
//! let drum = todo!();
//! let org = todo!();
//!
//! let mut player: OrgPlay<'_, Linear, AssetByRef<'_>> = OrgPlayBuilder::new()
//!     .with_sample_rate(44100)
//!     .with_interpolation(Linear)
//!     .with_asset(wavetable, drum) // Lifetime of them is now tied to AssetByRef<'_>
//!     .build(org) // Lifetime of `org` is now tied to OrgPlay<'_, ..>
//!     .expect("Invalid organya music");
//!
//! let mut buffer = [0.0_f32; 1024];
//! loop {
//!     player.synth_stereo(&mut buffer);
//!     // Process buffer and output
//! }
//! ```
//!
//! For owned [`OrgPlay`], use self-referential struct helpers like
//! [`self_cell`](https://crates.io/crates/self_cell) or [`ouroboros`](https://crates.io/crates/ouroboros).
//! See orgorg-player for example.
//!
//! # How to get data needed for synthesis
//! See [orgorg-player](https://github.com/kpqi5858/orgorg/tree/main/orgorg-player) project.
//! Run `orgorg-player dump` for Cave Story wavetable and drums.
//!
//! And see [`wdb`](https://github.com/kpqi5858/orgorg/blob/main/orgorg-player/src/wdb.rs)
//! module in orgorg-player for loading `soundbank.wdb`.
//!
//! # Cargo Features: `fma` and `fma-nightly`
//!
//! Uses Fused Multiply Add where possible, which might improve performance if the platform supports it.
//!
//! - `fma` requires `std`.
//! - `fma-nightly` does not require `std` but requires nightly compiler. It can be non-deterministic.
//!
//! # Cargo Features: `f32smp`
//!
//! Uses `f32` instead of `i8` for the type of drums and wavetable sample ([`OrgSmp`]),
//! which will improve performance on modern platforms at the cost of more memory usage for sample data.
//!
//! # Performance
//! It is programmed to take maximum advantage of LLVM's auto-vectorization thus
//! extremely fast on modern platforms. Plus it is optimized for embedded usage:
//!
//! - No dynamic memory allocation. Does not require `alloc`.
//! - No integer division.
//! - No `f64` arithmetic.
//! - No 64-bit arithmetic on 32-bit platform.
//! - Whole player state can be as small as 408 bytes + one pointer width.
//!
//! But with following caveats.
//!
//! - FPU should be present for maximum performance, since there are lots of `f32` arithmetic.
//! - This crate uses some unsafe to boost the performance.
//!   The author tried to ensure safety but, who knows. Feel free to audit the code.
//! - As you might guessed from generic [`OrgPlay`] type,
//!   constructing many variants of `OrgPlay` may lead to size bloat.
//!
//! If you want numbers, my x86 PC can synthesize Cave Story Main Theme at over 7800x speed.
//! (Linear Interpolation, 48000 Hz Stereo, `fma` & `f32smp`).

use core::{
    cmp,
    error::Error,
    fmt::{Debug, Display},
    marker::PhantomData,
    mem::MaybeUninit,
    num::Wrapping,
    ptr::NonNull,
};

const MASTER_VOLUME: f32 = 1.0 / (1 << 19) as f32;
const MAX_DRUM_LEN: usize = 500000;

/// Type of drums and wavetable data.
#[cfg(feature = "f32smp")]
pub type OrgSmp = f32;
/// Type of drums and wavetable data.
#[cfg(not(feature = "f32smp"))]
pub type OrgSmp = i8;

/// Provides original Cave Story wavetable and drum samples to [`OrgPlay`].
///
/// With this trait, it can play Org-02 musics that uses original Cave Story drum sound effects.
/// In other words, drum channel only plays wave 0, 2, 4, 5, 6, 8.
///
/// You don't need to implement this trait to use [`OrgPlay`],
/// as [`OrgPlayBuilder::with_asset`] will use default implementation
/// that holds references to the data.
pub trait CaveStoryAssetProvider {
    /// The original `wavetable.dat` file.
    fn wavetable(&self) -> &[OrgSmp; 25600];
    /// 6 pxt samples concatenated.
    ///
    /// Order is: fx96, fx97, fx9a, fx98, fx99, fx9b
    fn drum(&self) -> &[OrgSmp; 40000];
}

/// Provides wavetable and drum samples to [`OrgPlay`].
///
/// You don't really need to implement this trait yourself,
/// as [`Soundbank`] and [`CaveStoryAssetProvider`] provides implementation for this trait.
///
/// # Safety
/// - Return value of [`SoundbankProvider::drum`] must be consistent for given `idx` across all calls.
/// - If [`SoundbankProvider::drum`] returns `Some(slice)`, length of `slice` must be in `[1, 500000]`,
///   and its length must be consistent across all calls.
///
/// In other words, don't tamper with outputs using interior mutability or external source.
pub unsafe trait SoundbankProvider {
    /// The original `wavetable.dat` file, or 100 concatenated 256-length waves.
    fn wavetable(&self) -> &[OrgSmp; 25600];

    /// Get drum sample of `idx`, if valid.
    fn drum(&self, idx: u8) -> Option<&[OrgSmp]>;
}

// Safety: All function is consistent.
unsafe impl<T: CaveStoryAssetProvider> SoundbankProvider for T {
    #[inline(always)]
    fn wavetable(&self) -> &[OrgSmp; 25600] {
        CaveStoryAssetProvider::wavetable(self)
    }

    #[inline(always)]
    fn drum(&self, idx: u8) -> Option<&[OrgSmp]> {
        let drums = CaveStoryAssetProvider::drum(self);
        let range = match idx {
            0 => (0, 5000),
            2 => (5000, 10000),
            4 => (15000, 10000),
            5 => (25000, 1000),
            6 => (26000, 10000),
            8 => (36000, 4000),
            _ => return None,
        };
        // Avoid panic code. Compiler can't prove it.
        unsafe { Some(drums.get_unchecked(range.0..range.0 + range.1)) }
    }
}

/// Default provider used in [`OrgPlayBuilder::with_asset`]
#[derive(Debug)]
pub struct AssetByRef<'a>(&'a [OrgSmp; 25600], &'a [OrgSmp; 40000]);

impl CaveStoryAssetProvider for AssetByRef<'_> {
    #[inline(always)]
    fn wavetable(&self) -> &[OrgSmp; 25600] {
        self.0
    }

    #[inline(always)]
    fn drum(&self) -> &[OrgSmp; 40000] {
        self.1
    }
}

/// Custom soundbank by ref.
///
/// 43 drums will play Org-03 songs properly.
#[derive(Clone, Debug)]
pub struct Soundbank<'a> {
    wavetable: &'a [OrgSmp; 25600],
    drums: &'a [&'a [OrgSmp]],
}

impl<'a> Soundbank<'a> {
    /// Creates new Soundbank.
    ///
    /// - More than 255 `drums` is effectively ignored.
    /// - If length of a drum is not in `[1, 500000]`,
    ///   that particular drum is considered invalid and won't play a sound.
    pub fn new(wavetable: &'a [OrgSmp; 25600], drums: &'a [&'a [OrgSmp]]) -> Self {
        Self { wavetable, drums }
    }
}

// Safety: All function is consistent.
unsafe impl SoundbankProvider for Soundbank<'_> {
    #[inline(always)]
    fn wavetable(&self) -> &[OrgSmp; 25600] {
        self.wavetable
    }

    #[inline(always)]
    fn drum(&self, idx: u8) -> Option<&[OrgSmp]> {
        let drum = self.drums.get(idx as usize)?;
        (1..=MAX_DRUM_LEN).contains(&drum.len()).then_some(drum)
    }
}

/// Interpolation for Organya Music synthesis.
///
/// Keep in mind that these functions are called at audio rate.
/// You would like to put `#[inline]` and optimize them really well.
/// Specifically, it should be auto-vectorization friendly.
///
/// Implementer of `OrgInterpolation` must be ZST. Otherwise you will get compilation error.
/// It is meant to be stateless.
pub trait OrgInterpolation {
    /// How many samples prior to `pos` required by the interpolation.
    ///
    /// If the interpolation samples at `pos - N`,
    /// this should be set to `N` to ensure potential trailing non-zero values get written.
    ///
    /// Only relevant for drum.
    const INTERP_REMNANT: u32 = 0;

    /// Interpolate the `wave` from `(pos).(frac)`.
    ///
    /// `pos` should be wrapped by 256 (`& 0xff`) before indexing.
    fn wave(wave: &[OrgSmp; 256], pos: u32, frac: f32) -> f32;

    /// Interpolate the `drum` from `(pos).(frac)`.
    ///
    /// Out of bounds `drum` read should be 0.
    ///
    /// Note that length of `drum` should be in `[1, 500000]` and [`OrgPlay`] will ensure that.
    /// Implementations in [`interp_impls`] assume it and will return `0.0` if violated.
    fn drum(drum: &[OrgSmp], pos: u32, frac: f32) -> f32;
}

/// Builtin [`OrgInterpolation`] implementations.
pub mod interp_impls {
    /// Linear Interpolation. Fast.
    #[derive(Debug)]
    pub struct Linear;

    /// No Interpolation. Fastest.
    #[derive(Debug)]
    pub struct NoInterp;

    /// Lagrange Interpolation. Slow.
    #[derive(Debug)]
    pub struct Lagrange;
}

trait FmaUtils {
    fn fma(self, a: f32, b: f32) -> f32;
}

impl FmaUtils for f32 {
    #[allow(unreachable_code)]
    #[allow(clippy::needless_return)]
    #[inline(always)]
    fn fma(self, a: f32, b: f32) -> f32 {
        #[cfg(feature = "fma")]
        return self.mul_add(a, b);
        #[cfg(feature = "fma-nightly")]
        return self.algebraic_mul(a).algebraic_add(b);
        return (self * a) + b;
    }
}

mod _interp_impls {
    use crate::MAX_DRUM_LEN;

    use super::FmaUtils;
    use super::OrgInterpolation;
    use super::OrgSmp;
    use super::interp_impls::*;

    // Auto-vectorization friendly getter
    trait GatherUtils {
        fn get_or_zero(&self, idx: u32) -> f32;
        // Safety: In bounds
        unsafe fn get_as_f32(&self, idx: u32) -> f32;
    }

    impl GatherUtils for [f32] {
        #[inline(always)]
        fn get_or_zero(&self, idx: u32) -> f32 {
            // This check will be optimized out.
            if !(1..=MAX_DRUM_LEN).contains(&self.len()) {
                return 0.0;
            }
            let len = self.len() as u32 - 1;
            let cond = 0_u32.wrapping_sub((idx <= len) as u32);
            let actual_idx = idx.min(len);
            // https://github.com/llvm/llvm-project/issues/163023
            // Missed optimization with `zext nneg` index then gather
            // This emits two vgatherqd instructions, not ideal single vgatherdd.
            unsafe { core::hint::assert_unchecked(actual_idx <= i32::MAX as u32) };
            let value = unsafe { *self.get_unchecked(actual_idx as usize) };
            f32::from_bits(value.to_bits() & cond)
        }

        #[inline(always)]
        unsafe fn get_as_f32(&self, idx: u32) -> f32 {
            unsafe { *self.get_unchecked(idx as usize) }
        }
    }

    impl GatherUtils for [i8] {
        #[inline(always)]
        fn get_or_zero(&self, idx: u32) -> f32 {
            if !(1..=MAX_DRUM_LEN).contains(&self.len()) {
                return 0.0;
            }
            let len = self.len() as u32 - 1;
            let cond = 0_u8.wrapping_sub((idx <= len) as u8);
            let actual_idx = idx.min(len);
            unsafe { core::hint::assert_unchecked(actual_idx <= i32::MAX as u32) };
            let value = unsafe { *self.get_unchecked(actual_idx as usize) };
            (value & cond as i8) as f32
        }

        #[inline(always)]
        unsafe fn get_as_f32(&self, idx: u32) -> f32 {
            unsafe { *self.get_unchecked(idx as usize) as f32 }
        }
    }

    impl OrgInterpolation for Linear {
        #[inline(always)]
        fn wave(wave: &[OrgSmp; 256], pos: u32, frac: f32) -> f32 {
            unsafe {
                let idx1 = pos & 0xff;
                let sample1 = wave.get_as_f32(idx1);
                let idx2 = pos.wrapping_add(1) & 0xff;
                let sample2 = wave.get_as_f32(idx2);
                // The "imprecise" lerp (see Wikipedia Linear Interpolation).
                // Monotonic, and slightly fast over "precise" one.
                (sample2 - sample1).fma(frac, sample1)
            }
        }

        #[inline(always)]
        fn drum(drum: &[OrgSmp], pos: u32, frac: f32) -> f32 {
            let sample1 = drum.get_or_zero(pos);
            let sample2 = drum.get_or_zero(pos.wrapping_add(1));
            (sample2 - sample1).fma(frac, sample1)
        }
    }

    impl OrgInterpolation for NoInterp {
        #[inline(always)]
        fn wave(wave: &[OrgSmp; 256], pos: u32, _frac: f32) -> f32 {
            unsafe { wave.get_as_f32(pos & 0xff) }
        }

        #[inline(always)]
        fn drum(drum: &[OrgSmp], pos: u32, _frac: f32) -> f32 {
            drum.get_or_zero(pos)
        }
    }

    // I put raw C++ code in clang and translated llvm ir to Rust code lol.
    // Seems in C++ they try to employ fma by default (-ffp-contract=on).
    // But Rust strictly follows what programmer wrote.
    #[inline(always)]
    fn lagrange(s1: f32, s2: f32, s3: f32, s4: f32, frac: f32) -> f32 {
        let neg = -s1;
        let p0 = neg.fma(1.0 / 3.0, s3);
        let neg1 = -s2;
        let p1 = neg1.fma(1.0 / 2.0, p0);
        let neg2 = -s4;
        let p2 = neg2.fma(1.0 / 6.0, p1);
        let add = s1 + s3;
        let p3 = add.fma(1.0 / 2.0, neg1);
        let sub = s4 - s1;
        let sub4 = s2 - s3;
        let mul5 = sub4 * (1.0 / 2.0);
        let p4 = sub.fma(1.0 / 6.0, mul5);
        let p5 = p4.fma(frac, p3);
        let p6 = p5.fma(frac, p2);
        let p7 = p6.fma(frac, s2);
        return p7;
    }

    impl OrgInterpolation for Lagrange {
        const INTERP_REMNANT: u32 = 1;

        #[inline(always)]
        fn wave(wave: &[OrgSmp; 256], pos: u32, frac: f32) -> f32 {
            #[rustfmt::skip]
            let idx = [
                pos.wrapping_sub(1) & 0xff,
                pos                 & 0xff,
                pos.wrapping_add(1) & 0xff,
                pos.wrapping_add(2) & 0xff,
            ];
            unsafe {
                let s1 = wave.get_as_f32(idx[0]);
                let s2 = wave.get_as_f32(idx[1]);
                let s3 = wave.get_as_f32(idx[2]);
                let s4 = wave.get_as_f32(idx[3]);

                lagrange(s1, s2, s3, s4, frac)
            }
        }

        #[inline(always)]
        fn drum(drum: &[OrgSmp], pos: u32, frac: f32) -> f32 {
            #[rustfmt::skip]
            let idx = [
                pos.wrapping_sub(1),
                pos               ,
                pos.wrapping_add(1),
                pos.wrapping_add(2),
            ];
            let s1 = drum.get_or_zero(idx[0]);
            let s2 = drum.get_or_zero(idx[1]);
            let s3 = drum.get_or_zero(idx[2]);
            let s4 = drum.get_or_zero(idx[3]);

            lagrange(s1, s2, s3, s4, frac)
        }
    }

    #[cfg(test)]
    mod test {
        #[test]
        fn gather_utils() {
            use super::GatherUtils;

            let arr: &[f32] = &[1.0; 8];
            assert_eq!(arr.get_or_zero(0), 1.0);
            assert_eq!(arr.get_or_zero(8), 0.0);
            assert_eq!(unsafe { arr.get_as_f32(0) }, 1.0);

            let arr: &[i8] = &[1; 8];
            assert_eq!(arr.get_or_zero(0), 1.0);
            assert_eq!(arr.get_or_zero(8), 0.0);
            assert_eq!(unsafe { arr.get_as_f32(0) }, 1.0);

            let arr: &[f32] = &[];
            assert_eq!(arr.get_or_zero(0), 0.0);
            assert_eq!(arr.get_or_zero(8), 0.0);

            let arr: &[i8] = &[];
            assert_eq!(arr.get_or_zero(0), 0.0);
            assert_eq!(arr.get_or_zero(8), 0.0);
        }
    }
}

struct Event {
    note: u8,
    length: u8,
    volume: u8,
    panning: u8,
}

#[derive(Debug)]
struct Instrument<const DRUM: bool> {
    tuning: i16,
    pi: bool,
    // Supposedly the maximum number of events in a single instrument is 256.
    // Some incompatible(non-standard?) music can exceed that arbitrary limit.
    // So, be lenient here.
    n_events: u16,
    cur_event: u16,
    loop_event: u16,
    phase_inc: u32,
    // For DRUM, this is integer part of phase accumulator
    // For !DRUM, this is 8.24 phase accumulator
    phase_acc: u32,
    cur_pan: u8,
    cur_vol: u8,
    // Invariants:
    // - If n_events != 0, must point to valid wave
    wave_idx: u8,
    // For DRUM, this is fractional part of phase accumulator
    // For !DRUM, this is length
    cur_len_or_phase_acc: u32,
}

unsafe impl<const DRUM: bool> Send for Instrument<DRUM> {}
unsafe impl<const DRUM: bool> Sync for Instrument<DRUM> {}

// 8.24 fixed point arithmetic
const I24: u32 = 0x1000000;
const I24MASK: u32 = I24 - 1;
const F24: f32 = I24 as f32;

impl<const DRUM: bool> Instrument<DRUM> {
    // Safety: cur_event < n_events
    unsafe fn get_cur_event_beat(&self, ptr: NonNull<u8>) -> u32 {
        debug_assert!(self.cur_event < self.n_events);
        // Safety: See inst_data_ptr field comment
        unsafe { ptr.add(self.cur_event as usize * 4).cast().read_unaligned() }
    }

    // Safety: cur_event < n_events
    unsafe fn get_cur_event(&self, ptr: NonNull<u8>) -> Event {
        debug_assert!(self.cur_event < self.n_events);
        // Safety: See inst_data_ptr field comment
        unsafe {
            let n_events = self.n_events as usize;
            let inst_ptr = ptr.add(n_events * 4 + self.cur_event as usize);
            let note = inst_ptr.read();
            let length = inst_ptr.add(n_events).read();
            let volume = inst_ptr.add(n_events * 2).read();
            let panning = inst_ptr.add(n_events * 3).read();
            Event {
                note,
                length,
                volume,
                panning,
            }
        }
    }

    fn calculate_loop(&mut self, loop_start: u32, ptr: NonNull<u8>) {
        self.cur_event = 0;
        loop {
            if self.cur_event >= self.n_events {
                self.loop_event = self.n_events.saturating_sub(1);
                break;
            }
            let cur_beat = unsafe { self.get_cur_event_beat(ptr) };
            if cur_beat >= loop_start {
                self.loop_event = self.cur_event;
                break;
            }
            self.cur_event += 1;
        }
        self.cur_event = 0;
    }

    fn tick(&mut self, cur_beat: u32, loop_start: u32, rate_recip: f32, ptr: NonNull<u8>) {
        // There is no official documentation for .org file,
        // and these logics are not designed to handle it as leniently as possible.
        // It assumes that event is sorted by its beat, and no more event after loop_end.
        // But OrgMaker (the only official .org editor) output follows those rule.
        //
        // Unofficial reference
        // https://gist.github.com/fdeitylink/7fc9ddcc54b33971e5f505c8da2cfd28
        if cur_beat == loop_start {
            self.cur_event = self.loop_event;
        }
        if !DRUM && !self.pi {
            self.cur_len_or_phase_acc = self.cur_len_or_phase_acc.saturating_sub(1);
        }
        if self.cur_event >= self.n_events {
            return;
        }
        // Safety: Checked with above code
        let event = unsafe {
            let cur_event_beat = self.get_cur_event_beat(ptr);
            if cur_event_beat == cur_beat {
                self.get_cur_event(ptr)
            } else {
                return;
            }
        };
        self.cur_event += 1;
        if event.volume != 255 {
            self.cur_vol = event.volume;
        }
        if event.panning != 255 {
            const fn p(p: u8) -> u8 {
                const fn min(a: u8, b: u8) -> u8 {
                    if a > b { b } else { a }
                }
                let left = min(6, 12 - p);
                let right = min(6, p);
                (left << 4) | right
            }
            #[rustfmt::skip]
            const LUT: [u8; 13] = [ p(0), p(1), p(2), p(3), p(4), p(5), p(6), p(7), p(8), p(9), p(10), p(11), p(12) ];
            self.cur_pan = LUT[event.panning.min(12) as usize];
        }
        if event.note != 255 {
            self.cur_len_or_phase_acc = 0;
            self.phase_acc = 0;
            fn calc_inc(freq: u32, rate_recip: f32) -> Option<u32> {
                let res = (freq as i32 as f32) * rate_recip;
                if res >= 256.0 {
                    None
                } else {
                    unsafe {
                        let i = res.to_int_unchecked::<i32>() as u32;
                        let sub = res - i as f32;
                        Some((i << 24) | (sub * F24).to_int_unchecked::<i32>() as u32)
                    }
                }
            }
            if DRUM {
                let freq = event.note as u32 * 800 + 100;
                if let Some(inc) = calc_inc(freq, rate_recip) {
                    self.phase_inc = inc;
                }
            } else {
                const FRQ_TABLE: [i32; 12] =
                    [262, 277, 294, 311, 330, 349, 370, 392, 415, 440, 466, 494];
                let freq = FRQ_TABLE[(event.note % 12) as usize];
                let oct = 1 << (5 + (event.note / 12).min(7) as i32);
                let final_freq = (freq * oct) + (self.tuning - 1000) as i32;
                let phase_inc = calc_inc(final_freq as u32, rate_recip);
                if let Some(inc) = phase_inc {
                    self.phase_inc = inc;
                    self.cur_len_or_phase_acc = if self.pi {
                        // TODO: I dont know what is the accurate formula for pi instrument
                        (oct as u32 + 1) * 4 * 256
                    } else {
                        event.length as u32
                    };
                }
            }
        }
    }

    // This function is the critical part of overall performance.
    fn fill_buf<A: SoundbankProvider, I: OrgInterpolation, const MONO: bool>(
        &mut self,
        buf: &mut [f32],
        a: &A,
    ) {
        if !DRUM && self.cur_len_or_phase_acc == 0 {
            return;
        }
        if DRUM && self.phase_inc == 0 {
            return;
        }
        // Safety: See wave_idx field comment
        let cur_wave = unsafe {
            if DRUM {
                a.drum(self.wave_idx).unwrap_unchecked()
            } else {
                debug_assert!((0..100).contains(&self.wave_idx));
                let idx = self.wave_idx as usize * 256;
                let w = a.wavetable().as_ptr();
                core::slice::from_raw_parts(w.add(idx), 256)
            }
        };
        let vol = self.cur_vol as i32;
        // Integer multiplication then float cast is slightly faster
        let left = ((self.cur_pan >> 4) as i32 * vol) as f32 * MASTER_VOLUME;
        let right = ((self.cur_pan & 0b00001111) as i32 * vol) as f32 * MASTER_VOLUME;
        let mono = (((self.cur_pan >> 4) + (self.cur_pan & 0b00001111)) as i32 * vol) as f32
            * (MASTER_VOLUME / 2.0);

        let n = match (MONO, self.pi) {
            (true, false) => buf.len(),
            (false, false) => buf.len() / 2,
            (true, true) => cmp::min(buf.len(), self.cur_len_or_phase_acc as usize),
            (false, true) => cmp::min(buf.len() / 2, self.cur_len_or_phase_acc as usize),
        };
        let buf = unsafe {
            let n_len = if MONO { n } else { n * 2 };
            buf.get_unchecked_mut(0..n_len)
        };

        let inc_i = self.phase_inc >> 24;
        let wave_inc = self.phase_inc;
        let inc_sub_24 = self.phase_inc & I24MASK;

        let mut pos = Wrapping(self.phase_acc);
        let mut pos_sub = Wrapping(self.cur_len_or_phase_acc);

        for chunk in buf.chunks_mut(if MONO { 256 } else { 512 }) {
            let chunk_size = chunk.len() as u32 / if MONO { 1 } else { 2 };
            // Will pos_sub overflow at the last iteration in the loop below?
            let does_overflow = pos_sub.0.checked_add(inc_sub_24 * chunk_size).is_none();

            for chunk in chunk.chunks_exact_mut(if MONO { 1 } else { 2 }) {
                let sample = unsafe {
                    if DRUM {
                        let base_pos = pos.0 + (pos_sub.0 >> 24);
                        core::hint::assert_unchecked(
                            base_pos < MAX_DRUM_LEN as u32 + 256 * 256 + 256,
                        );
                        core::hint::assert_unchecked((1..=MAX_DRUM_LEN).contains(&cur_wave.len()));
                        let frac = (pos_sub.0 & I24MASK) as f32 / F24;
                        let val = I::drum(cur_wave, base_pos, frac);
                        pos_sub += inc_sub_24;
                        pos += inc_i;
                        val
                    } else {
                        let base_pos = pos.0 >> 24;
                        let frac = ((pos.0 & I24MASK) as f32) / F24;
                        let val = I::wave(cur_wave.try_into().unwrap(), base_pos, frac);
                        pos += wave_inc;
                        val
                    }
                };
                if MONO {
                    let v = &mut chunk[0];
                    *v = sample.fma(mono, *v);
                } else {
                    let v = &mut chunk[0];
                    *v = sample.fma(left, *v);
                    let v = &mut chunk[1];
                    *v = sample.fma(right, *v);
                }
            }
            if DRUM {
                pos += pos_sub.0 >> 24;
                // Add overflowed value here
                pos += does_overflow as u32 * 256;
                pos_sub.0 &= I24MASK;
                if pos.0 >= cur_wave.len() as u32 + I::INTERP_REMNANT {
                    self.phase_inc = 0;
                    return;
                }
            }
        }

        self.phase_acc = pos.0;
        self.cur_len_or_phase_acc = pos_sub.0;
        if !DRUM && self.pi {
            self.cur_len_or_phase_acc -= n as u32;
        }
    }
}

/// Playback option for [`OrgPlay`].
#[derive(Default, Clone, Copy, Debug)]
pub enum PlayTill {
    /// Play endlessly.
    #[default]
    Endless,
    /// Play until specified beat.
    ///
    /// If the specified beat is out of range for the song, it will behave like `Endless`.
    Beat(u32),
    /// Play until song loops.
    Loop,
}

/// Result of [`OrgPlay`] playback, according to [`PlayTill`] option.
#[derive(Debug)]
pub struct PlayResult(bool, usize);

impl PlayResult {
    /// Returns `true` if the playback have reached the end.
    pub fn reached_end(&self) -> bool {
        self.0
    }

    /// If the playback have reached the end, length of filled samples in the buffer.
    /// Rest of the buffer is filled with `0.0`.
    ///
    /// Else, this is always the full length of the buffer.
    pub fn filled_length(&self) -> usize {
        self.1
    }
}

/// `no_std` compatible Cave Story Organya Music Player.
#[derive(Debug)]
pub struct OrgPlay<'a, I: OrgInterpolation, A: SoundbankProvider> {
    song_data: NonNull<u8>,
    _song_data_ref: PhantomData<&'a [u8]>,
    sample_rate_recip: f32,
    samples_per_beat: i32,
    remaining_samples: i32,
    loop_start: u32,
    loop_end: u32,
    cur_beat: u32,
    wave_ins: [Instrument<false>; 8],
    drum_ins: [Instrument<true>; 8],
    asset: A,
    _i: PhantomData<I>,
}

/// Read-only state of one voice in the running renderer (Kog extension).
#[derive(Clone, Copy, Debug, Default)]
pub struct OrgChannelState {
    pub active: bool,
    pub wave: u8,
    pub tuning: i16,
    pub pi: bool,
    pub phase_increment: u32,
    pub volume: u8,
    pub pan_left: u8,
    pub pan_right: u8,
    pub row_event: bool,
    pub row_note: u8,
    pub row_length: u8,
    pub row_volume: u8,
    pub row_pan: u8,
}

unsafe impl<'a, I: OrgInterpolation, A: SoundbankProvider + Send> Send for OrgPlay<'a, I, A> {}
unsafe impl<'a, I: OrgInterpolation, A: SoundbankProvider + Sync> Sync for OrgPlay<'a, I, A> {}

/// Errors that may happen when parsing Organya music and creating [`OrgPlay`].
#[derive(Debug)]
pub enum OrgError {
    /// Invalid Organya song.
    InvalidOrg,
    /// Sample rate is too high.
    ///
    /// This can happen if Organya song is malformed,
    /// or specified sample rate is greater than 1,000,000.
    SampleRateTooHigh,
}

impl Display for OrgError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            OrgError::InvalidOrg => "Invalid Organya song",
            OrgError::SampleRateTooHigh => "Sample rate is too high",
        };
        f.write_str(s)
    }
}
impl Error for OrgError {}

impl<'a, I: OrgInterpolation, A: SoundbankProvider> OrgPlay<'a, I, A> {
    fn new(asset: A, song: &'a [u8], rate: u32) -> Result<Self, OrgError> {
        struct UnsafeReader<'a>(&'a [u8]);
        impl<'a> UnsafeReader<'a> {
            unsafe fn new(a: &'a [u8]) -> Self {
                Self(a)
            }

            fn read<const N: usize>(&self, offset: usize) -> [u8; N] {
                unsafe { self.0.get_unchecked(offset..offset + N).try_into().unwrap() }
            }

            fn read_u8(&self, offset: usize) -> u8 {
                self.read::<1>(offset)[0]
            }

            fn read_i16(&self, offset: usize) -> i16 {
                i16::from_le_bytes(self.read(offset))
            }

            fn read_u16(&self, offset: usize) -> u16 {
                u16::from_le_bytes(self.read(offset))
            }

            fn read_u32(&self, offset: usize) -> u32 {
                u32::from_le_bytes(self.read(offset))
            }
        }

        if song.len() < 114 {
            return Err(OrgError::InvalidOrg);
        }
        if !matches!(&song[0..6], b"Org-02" | b"Org-03") {
            return Err(OrgError::InvalidOrg);
        }
        // Safety: all following read is within index < 114
        let song_reader = unsafe { UnsafeReader::new(song) };
        let ms_per_beat = song_reader.read_u16(6);
        if ms_per_beat == 0 {
            return Err(OrgError::InvalidOrg);
        }
        let Some(samples_per_beat) = rate
            .checked_mul(ms_per_beat as u32)
            .and_then(|f| f.try_into().ok())
            .filter(|f: &i32| *f <= i32::MAX / 1000 * 1000)
        else {
            return Err(OrgError::SampleRateTooHigh);
        };
        let loop_start = song_reader.read_u32(10);
        let loop_end = song_reader.read_u32(14);
        if loop_end < loop_start {
            return Err(OrgError::InvalidOrg);
        }

        let rate_recip = 1.0 / rate as i32 as f32;

        let mut offset = 18;
        let mut ins_data_offset = 114;

        // core::array really needs try_from_fn, or array::try_map
        // Instrument does not allocate anything so no risk of memory leak when early returns.
        let mut wave_ins = [const { MaybeUninit::uninit() }; 8];
        let mut drum_ins = [const { MaybeUninit::uninit() }; 8];

        for val in &mut wave_ins {
            let wave = song_reader.read_u8(offset + 2);
            let valid_wave = (0..100).contains(&wave);
            let n_events = song_reader.read_u16(offset + 4);
            let pi = song_reader.read_u8(offset + 3) != 0;
            let inst_data_ptr = if n_events == 0 {
                NonNull::dangling()
            } else {
                let Some(inst_data) =
                    song.get(ins_data_offset..ins_data_offset + n_events as usize * 8)
                else {
                    return Err(OrgError::InvalidOrg);
                };
                // Safety: slice is always valid, and bound checked
                unsafe { NonNull::new_unchecked(inst_data.as_ptr() as *mut u8) }
            };
            let mut ret = Instrument {
                tuning: song_reader.read_i16(offset),
                pi,
                n_events,
                phase_inc: 0,
                phase_acc: 0,
                cur_pan: 0,
                cur_vol: 0,
                cur_len_or_phase_acc: 0,
                cur_event: 0,
                loop_event: 0,
                wave_idx: wave,
            };
            if valid_wave {
                ret.calculate_loop(loop_start, inst_data_ptr);
                // Initial ticking for beat 0, since synth function will start ticking at beat 1
                ret.tick(0, loop_start, rate_recip, inst_data_ptr);
            } else {
                ret.loop_event = u16::MAX;
                ret.cur_event = u16::MAX;
            }
            offset += 6;
            ins_data_offset += n_events as usize * 8;
            val.write(ret);
        }
        for val in &mut drum_ins {
            let wave = song_reader.read_u8(offset + 2);
            let valid_wave = asset.drum(wave).is_some();
            let n_events = song_reader.read_u16(offset + 4);
            let pi = song_reader.read_u8(offset + 3) != 0;
            let inst_data_ptr = if n_events == 0 {
                NonNull::dangling()
            } else {
                let Some(inst_data) =
                    song.get(ins_data_offset..ins_data_offset + n_events as usize * 8)
                else {
                    return Err(OrgError::InvalidOrg);
                };
                // Safety: slice is always valid, and bound checked
                unsafe { NonNull::new_unchecked(inst_data.as_ptr() as *mut u8) }
            };
            let mut ret = Instrument {
                tuning: song_reader.read_i16(offset),
                pi,
                n_events,
                phase_inc: 0,
                phase_acc: 0,
                cur_pan: 0,
                cur_vol: 0,
                cur_len_or_phase_acc: 0,
                cur_event: 0,
                loop_event: 0,
                wave_idx: wave,
            };
            if valid_wave {
                ret.calculate_loop(loop_start, inst_data_ptr);
                // Initial ticking for beat 0, since synth function will start ticking at beat 1
                ret.tick(0, loop_start, rate_recip, inst_data_ptr);
            } else {
                ret.loop_event = u16::MAX;
                ret.cur_event = u16::MAX;
            }
            offset += 6;
            ins_data_offset += n_events as usize * 8;
            val.write(ret);
        }

        // More data after song? Reject.
        if ins_data_offset != song.len() {
            return Err(OrgError::InvalidOrg);
        }

        let song_data = unsafe { NonNull::new_unchecked(song.as_ptr() as *mut u8).add(114) };

        Ok(Self {
            song_data,
            sample_rate_recip: rate_recip,
            samples_per_beat,
            remaining_samples: samples_per_beat,
            loop_start,
            loop_end,
            cur_beat: 0,
            // Safety: They are all initialized now.
            // TODO: Switch to array_assume_init when it lands
            wave_ins: unsafe {
                core::mem::transmute::<[MaybeUninit<Instrument<false>>; 8], [Instrument<false>; 8]>(
                    wave_ins,
                )
            },
            drum_ins: unsafe {
                core::mem::transmute::<[MaybeUninit<Instrument<true>>; 8], [Instrument<true>; 8]>(
                    drum_ins,
                )
            },
            asset,
            _song_data_ref: PhantomData,
            _i: PhantomData,
        })
    }

    /// Advance song and generate 1-channel mono audio data.
    ///
    /// Values can exceed `[-1, 1]` range on some songs.
    pub fn synth_mono(&mut self, buf: &mut [f32]) {
        self.synth_impl::<true>(buf, PlayTill::Endless);
    }

    /// Advance song and generate stereo interleaved audio data.
    ///
    /// Values can exceed `[-1, 1]` range on some songs.
    /// # Panics
    ///
    /// Panics if `buf.len()` is not multiple of 2.
    pub fn synth_stereo(&mut self, buf: &mut [f32]) {
        self.synth_impl::<false>(buf, PlayTill::Endless);
    }

    /// Advance song and generate 1-channel mono audio data, till specified position.
    ///
    /// Values can exceed `[-1, 1]` range on some songs.
    pub fn synth_mono_till(&mut self, buf: &mut [f32], till: PlayTill) -> PlayResult {
        self.synth_impl::<true>(buf, till)
    }

    /// Advance song and generate stereo interleaved audio data, till specified position.
    ///
    /// Values can exceed `[-1, 1]` range on some songs.
    /// # Panics
    ///
    /// Panics if `buf.len()` is not multiple of 2.
    pub fn synth_stereo_till(&mut self, buf: &mut [f32], till: PlayTill) -> PlayResult {
        self.synth_impl::<false>(buf, till)
    }

    fn synth_impl<const MONO: bool>(&mut self, buf: &mut [f32], till: PlayTill) -> PlayResult {
        if !MONO {
            assert!(buf.len().is_multiple_of(2));
        }
        // Just in case if user wants to play till loop_end, which is equivalent to PlayTill::Loop.
        let till = if let PlayTill::Beat(b) = till
            && b == self.loop_end
        {
            PlayTill::Loop
        } else {
            till
        };

        buf.fill(0.0);
        let mut filled_raw = 0;
        while filled_raw < buf.len() {
            if self.remaining_samples <= 0 {
                self.remaining_samples += self.samples_per_beat;
                self.cur_beat += 1;
                let looped;
                if self.cur_beat >= self.loop_end {
                    self.cur_beat = self.loop_start;
                    looped = true;
                } else {
                    looped = false;
                }
                let mut ptr = self.song_data;
                for w in &mut self.wave_ins {
                    w.tick(self.cur_beat, self.loop_start, self.sample_rate_recip, ptr);
                    ptr = unsafe { ptr.add(w.n_events as usize * 8) };
                }
                for w in &mut self.drum_ins {
                    w.tick(self.cur_beat, self.loop_start, self.sample_rate_recip, ptr);
                    ptr = unsafe { ptr.add(w.n_events as usize * 8) };
                }
                match till {
                    PlayTill::Endless => {}
                    PlayTill::Loop => {
                        if looped {
                            return PlayResult(true, filled_raw);
                        }
                    }
                    PlayTill::Beat(b) => {
                        if self.cur_beat == b {
                            return PlayResult(true, filled_raw);
                        }
                    }
                }
            }
            debug_assert!(self.remaining_samples > 0);
            let from_raw = filled_raw;
            // Seems compiler can't treat channel as const and optimize here.
            // let channel = if MONO { 1 } else { 2 };
            //
            // let to_fill_raw = cmp::min(
            //     libm::ceilf(self.remaining_samples) as usize * channel,
            //     buf.len() - filled_raw,
            // );
            // So, manual branching here.
            let to_fill_raw = if MONO {
                cmp::min(
                    (self.remaining_samples as u32).div_ceil(1000) as usize,
                    buf.len() - filled_raw,
                )
            } else {
                cmp::min(
                    (self.remaining_samples as u32).div_ceil(1000) as usize * 2,
                    buf.len() - filled_raw,
                )
            };
            // Seems compiler can't prove that no out of bounds will happen here as well.
            let fill_buffer = unsafe { buf.get_unchecked_mut(from_raw..from_raw + to_fill_raw) };
            for w in &mut self.wave_ins {
                w.fill_buf::<A, I, MONO>(fill_buffer, &self.asset);
            }
            for w in &mut self.drum_ins {
                w.fill_buf::<A, I, MONO>(fill_buffer, &self.asset);
            }
            filled_raw += to_fill_raw;
            // Same thing probably applies here
            if MONO {
                self.remaining_samples -= to_fill_raw as i32 * 1000;
            } else {
                self.remaining_samples -= to_fill_raw as i32 * 500;
            }
        }
        PlayResult(false, buf.len())
        // Multiplying MASTER_VOLUME in fill_buf is somewhat faster
        // buf.iter_mut().for_each(|f| *f *= MASTER_VOLUME);
    }

    /// Returns (Loop Start, Loop End).
    pub fn get_loop(&self) -> (u32, u32) {
        (self.loop_start, self.loop_end)
    }

    /// Returns current beat.
    pub fn get_beat(&self) -> u32 {
        self.cur_beat
    }

    /// Copy musical state without changing any renderer state (Kog extension).
    pub fn channel_state(&self) -> [OrgChannelState; 16] {
        fn read<const DRUM: bool>(instrument: &Instrument<DRUM>, ptr: NonNull<u8>, beat: u32) -> OrgChannelState {
            let mut state = OrgChannelState {
                active: if DRUM { instrument.phase_inc != 0 } else { instrument.cur_len_or_phase_acc != 0 },
                wave: instrument.wave_idx, tuning: instrument.tuning, pi: instrument.pi,
                phase_increment: instrument.phase_inc, volume: instrument.cur_vol,
                pan_left: instrument.cur_pan >> 4, pan_right: instrument.cur_pan & 15,
                row_note: 255, row_volume: 255, row_pan: 255, ..OrgChannelState::default()
            };
            if instrument.cur_event > 0 && instrument.cur_event <= instrument.n_events {
                let index = usize::from(instrument.cur_event - 1);
                // Same bounds and layout as get_cur_event_beat/get_cur_event.
                // The song and instrument event counts were checked at load.
                unsafe {
                    if ptr.add(index * 4).cast::<u32>().read_unaligned() == beat {
                        let n = usize::from(instrument.n_events);
                        let event = ptr.add(n * 4 + index);
                        state.row_event = true;
                        state.row_note = event.read();
                        state.row_length = event.add(n).read();
                        state.row_volume = event.add(n * 2).read();
                        state.row_pan = event.add(n * 3).read();
                    }
                }
            }
            state
        }
        let mut ptr = self.song_data;
        core::array::from_fn(|i| {
            let (state, count) = if i < 8 {
                (read(&self.wave_ins[i], ptr, self.cur_beat), self.wave_ins[i].n_events)
            } else {
                (read(&self.drum_ins[i - 8], ptr, self.cur_beat), self.drum_ins[i - 8].n_events)
            };
            // Advance over the same checked event arrays used by synthesis.
            ptr = unsafe { ptr.add(usize::from(count) * 8) };
            state
        })
    }

    // TODO: Seek function (Will be expensive)
}

/// Builder for [`OrgPlay`].
#[derive(Debug)]
pub struct OrgPlayBuilder<I, A>(PhantomData<I>, A, u32);

impl OrgPlayBuilder<(), ()> {
    /// Creates new OrgPlayBuilder.
    /// Initial default is [`Linear`](crate::interp_impls::Linear) interpolation and sample rate of 48000Hz.
    ///
    /// Provide soundbank by:
    /// - [`with_soundbank`](Self::with_soundbank)
    /// - [`with_soundbank_provider`](Self::with_soundbank_provider)
    ///
    /// Or, provide original Cave Story wavetable and drums by:
    /// - [`with_asset`](Self::with_asset)
    ///
    /// Otherwise it is compile error to call [`build`](Self::build).
    pub fn new() -> OrgPlayBuilder<crate::interp_impls::Linear, ()> {
        OrgPlayBuilder(PhantomData, (), 48000)
    }
}

impl<I, A> OrgPlayBuilder<I, A> {
    /// Set interpolation.
    pub fn with_interpolation<I2: OrgInterpolation>(self, _: I2) -> OrgPlayBuilder<I2, A> {
        const {
            assert!(
                core::mem::size_of::<I2>() == 0,
                "Implementer of OrgInterpolation must be ZST"
            );
        }
        OrgPlayBuilder(PhantomData, self.1, self.2)
    }

    /// Set sample rate.
    ///
    /// Note that sample rate over 1,000,000 may result in [`OrgError::SampleRateTooHigh`].
    /// # Panics
    ///
    /// Panics if `rate` is less than 1000.
    pub fn with_sample_rate(self, rate: u32) -> OrgPlayBuilder<I, A> {
        assert!(rate >= 1000);
        OrgPlayBuilder(self.0, self.1, rate)
    }

    /// Provide original Cave Story wavetable and drums for soundbank.
    ///
    /// Will only properly play songs with original Cave Story drum sound effects.
    /// See [`CaveStoryAssetProvider`] for more information.
    pub fn with_asset<'a>(
        self,
        wavetable: &'a [OrgSmp; 25600],
        drum: &'a [OrgSmp; 40000],
    ) -> OrgPlayBuilder<I, AssetByRef<'a>> {
        self.with_soundbank_provider(AssetByRef(wavetable, drum))
    }

    /// Provide [`Soundbank`].
    pub fn with_soundbank(self, a: Soundbank) -> OrgPlayBuilder<I, Soundbank> {
        self.with_soundbank_provider(a)
    }

    /// Provide [`SoundbankProvider`].
    pub fn with_soundbank_provider<A2: SoundbankProvider>(self, a: A2) -> OrgPlayBuilder<I, A2> {
        OrgPlayBuilder(PhantomData, a, self.2)
    }
}

impl<I, A> OrgPlayBuilder<I, A>
where
    I: OrgInterpolation,
    A: SoundbankProvider,
{
    /// Try to build [`OrgPlay`].
    pub fn build<'a>(self, song: &'a [u8]) -> Result<OrgPlay<'a, I, A>, OrgError> {
        OrgPlay::<I, A>::new(self.1, song, self.2)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn inst_loop() {
        let mut inst = Instrument::<false> {
            tuning: 0,
            pi: false,
            n_events: 4,
            cur_event: 0,
            loop_event: 0,
            phase_inc: 0,
            phase_acc: 0,
            cur_pan: 0,
            cur_vol: 0,
            wave_idx: 0,
            cur_len_or_phase_acc: 0,
        };

        let notes: [u32; 4] = [0, 1, 2, 3];
        inst.calculate_loop(0, unsafe {
            NonNull::new_unchecked(notes.as_ptr() as *const u8 as *mut u8)
        });
        assert_eq!(inst.loop_event, 0);

        let notes: [u32; 4] = [0, 1, 2, 3];
        inst.calculate_loop(4, unsafe {
            NonNull::new_unchecked(notes.as_ptr() as *const u8 as *mut u8)
        });
        assert_eq!(inst.loop_event, 3);

        let notes: [u32; 4] = [0, 2, 4, 6];
        inst.calculate_loop(3, unsafe {
            NonNull::new_unchecked(notes.as_ptr() as *const u8 as *mut u8)
        });
        assert_eq!(inst.loop_event, 2);

        let notes: [u32; 4] = [0, 2, 4, 6];
        inst.calculate_loop(2, unsafe {
            NonNull::new_unchecked(notes.as_ptr() as *const u8 as *mut u8)
        });
        assert_eq!(inst.loop_event, 1);

        let notes: [u32; 4] = [2, 4, 6, 8];
        inst.calculate_loop(1, unsafe {
            NonNull::new_unchecked(notes.as_ptr() as *const u8 as *mut u8)
        });
        assert_eq!(inst.loop_event, 0);
    }

    #[test]
    fn drum_synth_overflow() {
        let test_sample = core::array::from_fn(|i| i as OrgSmp);
        struct TestSoundbank([OrgSmp; 258]);
        unsafe impl SoundbankProvider for TestSoundbank {
            fn wavetable(&self) -> &[OrgSmp; 25600] {
                unreachable!()
            }

            fn drum(&self, _idx: u8) -> Option<&[OrgSmp]> {
                Some(&self.0)
            }
        }
        let soundbank = TestSoundbank(test_sample);

        let mut inst = Instrument::<true> {
            tuning: 0,
            pi: false,
            n_events: 0,
            cur_event: 0,
            loop_event: 0,
            phase_inc: 0xffffff,
            phase_acc: 0,
            cur_pan: 0b00010001,
            cur_vol: 100,
            wave_idx: 0,
            cur_len_or_phase_acc: 0xffffff,
        };
        let mut buf = [0.0; 257];
        inst.fill_buf::<_, interp_impls::NoInterp, true>(&mut buf, &soundbank);
        assert_eq!(inst.phase_acc, 257);
        // (phase_inc + (phase_acc * 257)) % (2^24)
        assert_eq!(inst.cur_len_or_phase_acc, 0xffffff - 257);
        #[cfg(feature = "f32smp")]
        assert!(buf.windows(2).all(|a| { a[0] < a[1] }));
    }
}
