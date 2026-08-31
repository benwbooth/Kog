//! Safe ownership wrapper around Cog's OPL3Windows/Nuked OPL3 engine.

use std::ptr::NonNull;

#[repr(C)]
struct KogOpl3w {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_opl3w_create(sample_rate: u32) -> *mut KogOpl3w;
    fn kog_opl3w_destroy(synth: *mut KogOpl3w);
    fn kog_opl3w_write(synth: *mut KogOpl3w, packed_midi: u32);
    fn kog_opl3w_generate(synth: *mut KogOpl3w, stereo: *mut i16, frames: u32);
}

pub struct Opl3WindowsSynth {
    handle: NonNull<KogOpl3w>,
}

impl Opl3WindowsSynth {
    pub fn new(sample_rate: u32) -> Result<Self, String> {
        let handle = unsafe { kog_opl3w_create(sample_rate) };
        let handle = NonNull::new(handle)
            .ok_or_else(|| "initializing Cog's OPL3Windows synthesizer failed".to_owned())?;
        Ok(Self { handle })
    }

    pub fn write_packed(&mut self, packed_midi: u32) {
        unsafe { kog_opl3w_write(self.handle.as_ptr(), packed_midi) };
    }

    pub fn generate(&mut self, stereo: &mut [i16]) -> Result<(), String> {
        if !stereo.len().is_multiple_of(2) {
            return Err("OPL3 output buffer must contain stereo sample pairs".to_owned());
        }
        let frames = u32::try_from(stereo.len() / 2)
            .map_err(|_| "OPL3 output buffer is too large".to_owned())?;
        unsafe { kog_opl3w_generate(self.handle.as_ptr(), stereo.as_mut_ptr(), frames) };
        Ok(())
    }
}

// The native synthesizer has no shared mutable global state. This wrapper owns
// its handle exclusively and rodio moves a Source between threads as a unit.
unsafe impl Send for Opl3WindowsSynth {}

impl Drop for Opl3WindowsSynth {
    fn drop(&mut self) {
        unsafe { kog_opl3w_destroy(self.handle.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opl3windows_generates_pcm_for_a_note() {
        let mut synth = Opl3WindowsSynth::new(48_000).expect("OPL3Windows synthesizer");
        synth.write_packed(0x0000c0);
        synth.write_packed(0x643c90);
        let mut pcm = vec![0_i16; 4_800 * 2];
        synth.generate(&mut pcm).expect("render OPL3Windows PCM");
        assert!(pcm.iter().any(|sample| *sample != 0));
    }
}
