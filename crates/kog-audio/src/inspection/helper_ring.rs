//! Bounded telemetry transport for standalone native renderer processes.
use super::{Monitor, Producer, native};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
const VOICES: usize = 64;
const SLOTS: u64 = 256;
const SLOT_BYTES: u64 = 32 + (VOICES * std::mem::size_of::<native::Voice>()) as u64;

pub(crate) struct HelperRing {
    file: File,
    path: tempfile::NamedTempFile,
    next: u64,
    producer: Option<Producer>,
}
impl HelperRing {
    pub fn new() -> Result<Self, String> {
        let path = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        let file = path.reopen().map_err(|e| e.to_string())?;
        Ok(Self {
            file,
            path,
            next: 1,
            producer: None,
        })
    }
    pub fn path(&self) -> &std::path::Path {
        self.path.path()
    }
    pub fn inspect(&mut self, monitor: &Monitor, sample_rate: u32) {
        self.producer = Some(monitor.producer(sample_rate));
        self.drain();
    }
    pub fn drain(&mut self) {
        let Some(producer) = &self.producer else {
            return;
        };
        let file = &mut self.file;
        let read_u64 = |file: &mut File, offset| -> std::io::Result<u64> {
            file.seek(SeekFrom::Start(offset))?;
            let mut bytes = [0; 8];
            file.read_exact(&mut bytes)?;
            Ok(u64::from_le_bytes(bytes))
        };
        let Ok(latest) = read_u64(file, 0) else {
            return;
        };
        if latest < self.next {
            return;
        }
        self.next = self.next.max(latest.saturating_sub(SLOTS - 1));
        while self.next <= latest {
            let offset = 8 + ((self.next - 1) % SLOTS) * SLOT_BYTES;
            if read_u64(file, offset).ok() != Some(self.next) {
                break;
            }
            let mut header = [0; 16];
            if file.read_exact(&mut header).is_err() {
                break;
            }
            let time = f64::from_le_bytes(header[..8].try_into().unwrap());
            let count = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
            if count > VOICES || !time.is_finite() || time < 0.0 {
                break;
            }
            let mut voices = vec![native::Voice::default(); count];
            // KogVoice consists entirely of integer, float, and byte fields;
            // every bit pattern is valid. Producer and helper share this ABI.
            let bytes = unsafe {
                std::slice::from_raw_parts_mut(
                    voices.as_mut_ptr().cast::<u8>(),
                    count * std::mem::size_of::<native::Voice>(),
                )
            };
            if file.read_exact(bytes).is_err() {
                break;
            }
            if read_u64(file, offset + SLOT_BYTES - 8).ok() != Some(self.next)
                || read_u64(file, offset).ok() != Some(self.next)
            {
                break;
            }
            producer.publish_at(native::frame(&voices), time);
            self.next += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn inspection_ring_ignores_incomplete_slots_and_wraps() {
        assert_eq!(std::mem::size_of::<native::Voice>(), 664);
        let mut ring = HelperRing::new().unwrap();
        let monitor = Monitor::default();
        ring.inspect(&monitor, 48000);
        let mut writer = File::options().write(true).open(ring.path()).unwrap();
        for sequence in 1u64..=300 {
            let offset = 8 + ((sequence - 1) % SLOTS) * SLOT_BYTES;
            writer.seek(SeekFrom::Start(offset)).unwrap();
            writer.write_all(&sequence.to_le_bytes()).unwrap();
            writer
                .write_all(&(sequence as f64 / 200.0).to_le_bytes())
                .unwrap();
            writer.write_all(&0u64.to_le_bytes()).unwrap();
            writer
                .seek(SeekFrom::Start(offset + SLOT_BYTES - 8))
                .unwrap();
            writer.write_all(&sequence.to_le_bytes()).unwrap();
        }
        writer.seek(SeekFrom::Start(0)).unwrap();
        writer.write_all(&300u64.to_le_bytes()).unwrap();
        ring.drain();
        assert_eq!(ring.next, 301);
        assert_eq!(
            monitor
                .recording_frames(None, std::time::Duration::from_secs(2))
                .len(),
            256
        );
        writer.seek(SeekFrom::Start(0)).unwrap();
        writer.write_all(&301u64.to_le_bytes()).unwrap();
        ring.drain();
        assert_eq!(ring.next, 301);
        assert!(writer.metadata().unwrap().len() <= 8 + SLOTS * SLOT_BYTES);
    }
}
