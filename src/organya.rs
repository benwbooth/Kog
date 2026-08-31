//! Owned Organya renderer built around the MIT-licensed `orgorg` crate.
//!
//! Organya's original wavetable and PixTone drums are Cave Story assets, so
//! Kog deliberately discovers a user-supplied bank instead of embedding them.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use directories::ProjectDirs;
use orgorg::{OrgPlay, OrgPlayBuilder, SoundbankProvider, interp_impls::Lagrange};
use self_cell::self_cell;

const WAVETABLE_SAMPLES: usize = 100 * 256;
const CAVE_STORY_DRUM_SAMPLES: usize = 40_000;
const MAX_DRUM_SAMPLES: usize = 500_000;
const MAX_DRUMS: usize = 256;
const MAX_DURATION: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Clone, Copy, Debug)]
struct OrgHeader {
    version: [u8; 6],
    milliseconds_per_beat: u16,
    loop_start: u32,
    loop_end: u32,
}

impl OrgHeader {
    fn parse(data: &[u8], path: &Path) -> Result<Self, String> {
        let version: [u8; 6] = data
            .get(..6)
            .and_then(|value| value.try_into().ok())
            .ok_or_else(|| format!("{} is too short to be an Organya file", path.display()))?;
        if !matches!(&version, b"Org-02" | b"Org-03") {
            return Err(format!(
                "{} is not supported Org-02 or Org-03 data",
                path.display()
            ));
        }
        let milliseconds_per_beat = read_u16(data, 6)
            .filter(|value| *value != 0)
            .ok_or_else(|| format!("{} has an invalid Organya tempo", path.display()))?;
        let loop_start = read_u32(data, 10)
            .ok_or_else(|| format!("{} has a truncated Organya header", path.display()))?;
        let loop_end = read_u32(data, 14)
            .ok_or_else(|| format!("{} has a truncated Organya header", path.display()))?;
        if loop_end < loop_start {
            return Err(format!(
                "{} has invalid Organya loop points",
                path.display()
            ));
        }
        Ok(Self {
            version,
            milliseconds_per_beat,
            loop_start,
            loop_end,
        })
    }

    fn codec_name(self) -> &'static str {
        if &self.version == b"Org-03" {
            "Organya Org-03"
        } else {
            "Organya Org-02"
        }
    }
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[derive(Debug)]
struct SoundbankData {
    samples: Box<[i8]>,
    drum_ranges: Box<[Option<(usize, usize)>]>,
}

impl SoundbankData {
    fn wavetable(&self) -> &[i8; WAVETABLE_SAMPLES] {
        self.samples[..WAVETABLE_SAMPLES]
            .try_into()
            .expect("validated Organya wavetable length")
    }

    fn drum(&self, index: u8) -> Option<&[i8]> {
        let (start, end) = self.drum_ranges.get(usize::from(index))?.as_ref()?;
        self.samples.get(*start..*end)
    }
}

#[derive(Clone)]
struct PlayerOwner {
    song: Arc<[u8]>,
    soundbank: Arc<SoundbankData>,
}

#[derive(Clone, Copy, Debug)]
struct BorrowedSoundbank<'a>(&'a SoundbankData);

// Safety: the immutable sample and range data cannot change during playback,
// and every returned drum slice has already been length-validated.
unsafe impl SoundbankProvider for BorrowedSoundbank<'_> {
    fn wavetable(&self) -> &[i8; WAVETABLE_SAMPLES] {
        self.0.wavetable()
    }

    fn drum(&self, index: u8) -> Option<&[i8]> {
        self.0.drum(index)
    }
}

type BorrowedPlayer<'a> = OrgPlay<'a, Lagrange, BorrowedSoundbank<'a>>;

self_cell!(
    struct PlayerCell {
        owner: PlayerOwner,

        #[covariant]
        dependent: BorrowedPlayer,
    }
);

fn build_player(
    song: Arc<[u8]>,
    soundbank: Arc<SoundbankData>,
    sample_rate: u32,
) -> Result<PlayerCell, String> {
    PlayerCell::try_new(PlayerOwner { song, soundbank }, |owner| {
        OrgPlayBuilder::new()
            .with_sample_rate(sample_rate)
            .with_interpolation(Lagrange)
            .with_soundbank_provider(BorrowedSoundbank(&owner.soundbank))
            .build(&owner.song)
            .map_err(|error| error.to_string())
    })
}

pub struct Organya {
    song: Arc<[u8]>,
    soundbank: Arc<SoundbankData>,
    player: PlayerCell,
    header: OrgHeader,
    sample_rate: u32,
    nominal_frames: u64,
    fade_frames: u64,
    total_frames: u64,
    position_frames: u64,
}

impl Organya {
    pub fn open(
        path: &Path,
        sample_rate: u32,
        loop_count: u32,
        fade: Duration,
    ) -> Result<Self, String> {
        let song =
            std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
        let soundbank = discover_soundbank(path)?;
        Self::from_parts(song, soundbank, path, sample_rate, loop_count, fade)
    }

    fn from_parts(
        song: Vec<u8>,
        soundbank: SoundbankData,
        path: &Path,
        sample_rate: u32,
        loop_count: u32,
        fade: Duration,
    ) -> Result<Self, String> {
        let header = OrgHeader::parse(&song, path)?;
        let loop_beats = header.loop_end - header.loop_start;
        let total_beats = u128::from(header.loop_start)
            .checked_add(u128::from(loop_beats) * u128::from(loop_count))
            .ok_or_else(|| format!("{} has an excessive Organya duration", path.display()))?;
        let nominal_numerator = total_beats
            .checked_mul(u128::from(header.milliseconds_per_beat))
            .and_then(|value| value.checked_mul(u128::from(sample_rate)))
            .ok_or_else(|| format!("{} has an excessive Organya duration", path.display()))?;
        let nominal_frames = u64::try_from(nominal_numerator.div_ceil(1_000))
            .map_err(|_| format!("{} has an excessive Organya duration", path.display()))?;
        let fade_frames = frames_from_duration(fade, sample_rate)?;
        let total_frames = nominal_frames
            .checked_add(fade_frames)
            .ok_or_else(|| format!("{} has an excessive Organya duration", path.display()))?;
        if duration_from_frames(total_frames, sample_rate) > MAX_DURATION {
            return Err(format!(
                "{} exceeds Kog's two-hour Organya safety limit",
                path.display()
            ));
        }

        let song: Arc<[u8]> = song.into();
        let soundbank = Arc::new(soundbank);
        let player = build_player(song.clone(), soundbank.clone(), sample_rate)
            .map_err(|error| format!("opening {} with orgorg: {error}", path.display()))?;
        Ok(Self {
            song,
            soundbank,
            player,
            header,
            sample_rate,
            nominal_frames,
            fade_frames,
            total_frames,
            position_frames: 0,
        })
    }

    pub fn duration(&self) -> Duration {
        duration_from_frames(self.total_frames, self.sample_rate)
    }

    pub fn codec_name(&self) -> &'static str {
        self.header.codec_name()
    }

    #[cfg(test)]
    fn loop_points(&self) -> (u32, u32) {
        (self.header.loop_start, self.header.loop_end)
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        if !output.len().is_multiple_of(2) {
            return Err("Organya output must contain stereo sample pairs".to_owned());
        }
        let requested_frames = output.len() / 2;
        let remaining = self.total_frames.saturating_sub(self.position_frames);
        let frames = requested_frames.min(usize::try_from(remaining).unwrap_or(usize::MAX));
        if frames == 0 {
            return Ok(0);
        }

        let samples = &mut output[..frames * 2];
        self.player
            .with_dependent_mut(|_, player| player.synth_stereo(samples));
        self.apply_fade(samples);
        self.position_frames += frames as u64;
        Ok(frames)
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = frames_from_duration(position.min(self.duration()), self.sample_rate)?;
        self.player = build_player(self.song.clone(), self.soundbank.clone(), self.sample_rate)?;
        self.position_frames = 0;

        let mut scratch = vec![0.0_f32; 4_096 * 2];
        while self.position_frames < target {
            let frames = usize::try_from((target - self.position_frames).min(4_096))
                .expect("bounded Organya seek chunk fits usize");
            self.player
                .with_dependent_mut(|_, player| player.synth_stereo(&mut scratch[..frames * 2]));
            self.position_frames += frames as u64;
        }
        Ok(duration_from_frames(target, self.sample_rate))
    }

    fn apply_fade(&self, samples: &mut [f32]) {
        if self.fade_frames == 0 {
            return;
        }
        for (frame_index, frame) in samples.chunks_exact_mut(2).enumerate() {
            let absolute_frame = self.position_frames + frame_index as u64;
            if absolute_frame < self.nominal_frames {
                continue;
            }
            let remaining = self.total_frames.saturating_sub(absolute_frame);
            let gain = remaining as f32 / self.fade_frames as f32;
            frame[0] *= gain;
            frame[1] *= gain;
        }
    }
}

fn discover_soundbank(song_path: &Path) -> Result<SoundbankData, String> {
    if let Some(explicit) = std::env::var_os("KOG_ORGANYA_SOUNDBANK") {
        let explicit = PathBuf::from(explicit);
        return load_soundbank_path(&explicit).map_err(|error| {
            format!(
                "loading KOG_ORGANYA_SOUNDBANK {}: {error}",
                explicit.display()
            )
        });
    }

    let mut directories = Vec::new();
    if let Some(parent) = song_path.parent() {
        directories.push(parent.to_owned());
    }
    if let Some(project) = ProjectDirs::from("org", "Kog", "Kog") {
        directories.push(project.config_dir().join("organya"));
        directories.push(project.data_local_dir().join("organya"));
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        directories.push(parent.join("organya"));
    }

    for directory in directories {
        if let Some(soundbank) = load_soundbank_directory(&directory)? {
            return Ok(soundbank);
        }
    }

    Err(format!(
        "Organya playback for {} needs user-supplied synthesis assets. Put soundbank.wdb or wavetable.dat plus drums.dat beside the song, install them in Kog's organya configuration directory, or set KOG_ORGANYA_SOUNDBANK",
        song_path.display()
    ))
}

fn load_soundbank_path(path: &Path) -> Result<SoundbankData, String> {
    if path.is_dir() {
        return load_soundbank_directory(path)?.ok_or_else(|| {
            format!(
                "{} contains neither soundbank.wdb nor wavetable.dat plus drums.dat",
                path.display()
            )
        });
    }
    if !path.is_file() {
        return Err(format!(
            "{} is not a readable file or directory",
            path.display()
        ));
    }
    if path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("wavetable.dat") || name.eq_ignore_ascii_case("drums.dat")
        })
    {
        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        return load_dat_pair(directory);
    }
    load_wdb(path)
}

fn load_soundbank_directory(directory: &Path) -> Result<Option<SoundbankData>, String> {
    let wdb = directory.join("soundbank.wdb");
    if wdb.is_file() {
        return load_wdb(&wdb).map(Some);
    }
    let wavetable = directory.join("wavetable.dat");
    let drums = directory.join("drums.dat");
    if wavetable.is_file() || drums.is_file() {
        return load_dat_pair(directory).map(Some);
    }
    Ok(None)
}

fn load_wdb(path: &Path) -> Result<SoundbankData, String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
    if bytes.len() < WAVETABLE_SAMPLES {
        return Err(format!(
            "{} has a truncated Organya wavetable",
            path.display()
        ));
    }

    let mut samples = Vec::with_capacity(bytes.len());
    samples.extend(
        bytes[..WAVETABLE_SAMPLES]
            .iter()
            .map(|sample| *sample as i8),
    );
    let mut drum_ranges = Vec::new();
    let mut offset = WAVETABLE_SAMPLES;
    while offset < bytes.len() {
        if drum_ranges.len() == MAX_DRUMS {
            return Err(format!(
                "{} contains more than 256 Organya drums",
                path.display()
            ));
        }
        let length = read_u32(&bytes, offset)
            .ok_or_else(|| format!("{} has a truncated drum length", path.display()))?
            as usize;
        if !(1..=MAX_DRUM_SAMPLES).contains(&length) {
            return Err(format!(
                "{} has an invalid Organya drum length of {length}",
                path.display()
            ));
        }
        let data_start = offset + 4;
        let data_end = data_start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| format!("{} has truncated Organya drum data", path.display()))?;
        let sample_start = samples.len();
        samples.extend(
            bytes[data_start..data_end]
                .iter()
                .map(|sample| sample.wrapping_sub(0x80) as i8),
        );
        drum_ranges.push(Some((sample_start, samples.len())));
        offset = data_end;
    }
    Ok(SoundbankData {
        samples: samples.into_boxed_slice(),
        drum_ranges: drum_ranges.into_boxed_slice(),
    })
}

fn load_dat_pair(directory: &Path) -> Result<SoundbankData, String> {
    let wavetable_path = directory.join("wavetable.dat");
    let drums_path = directory.join("drums.dat");
    let wavetable = std::fs::read(&wavetable_path)
        .map_err(|error| format!("reading {}: {error}", wavetable_path.display()))?;
    let drums = std::fs::read(&drums_path)
        .map_err(|error| format!("reading {}: {error}", drums_path.display()))?;
    if wavetable.len() != WAVETABLE_SAMPLES {
        return Err(format!(
            "{} must contain exactly {WAVETABLE_SAMPLES} samples",
            wavetable_path.display()
        ));
    }
    if drums.len() != CAVE_STORY_DRUM_SAMPLES {
        return Err(format!(
            "{} must contain exactly {CAVE_STORY_DRUM_SAMPLES} samples",
            drums_path.display()
        ));
    }

    let mut samples = Vec::with_capacity(WAVETABLE_SAMPLES + CAVE_STORY_DRUM_SAMPLES);
    samples.extend(wavetable.into_iter().map(|sample| sample as i8));
    samples.extend(drums.into_iter().map(|sample| sample as i8));
    let base = WAVETABLE_SAMPLES;
    let drum_ranges = vec![
        Some((base, base + 5_000)),
        None,
        Some((base + 5_000, base + 15_000)),
        None,
        Some((base + 15_000, base + 25_000)),
        Some((base + 25_000, base + 26_000)),
        Some((base + 26_000, base + 36_000)),
        None,
        Some((base + 36_000, base + 40_000)),
    ];
    Ok(SoundbankData {
        samples: samples.into_boxed_slice(),
        drum_ranges: drum_ranges.into_boxed_slice(),
    })
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "Organya duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond Organya duration fits u32"),
    )
}

#[cfg(test)]
pub fn test_org_bytes() -> Vec<u8> {
    let mut song = Vec::with_capacity(122);
    song.extend_from_slice(b"Org-02");
    song.extend_from_slice(&100_u16.to_le_bytes());
    song.extend_from_slice(&[4, 4]);
    song.extend_from_slice(&0_u32.to_le_bytes());
    song.extend_from_slice(&4_u32.to_le_bytes());
    for instrument in 0..16 {
        song.extend_from_slice(&0_i16.to_le_bytes());
        song.push(0);
        song.push(0);
        let event_count = if instrument == 0 { 1_u16 } else { 0_u16 };
        song.extend_from_slice(&event_count.to_le_bytes());
    }
    song.extend_from_slice(&0_u32.to_le_bytes());
    song.extend_from_slice(&[48, 4, 200, 6]);
    assert_eq!(song.len(), 122);
    song
}

#[cfg(test)]
pub fn test_soundbank_wdb_bytes() -> Vec<u8> {
    let mut bank = vec![0_u8; WAVETABLE_SAMPLES];
    for (index, sample) in bank[..256].iter_mut().enumerate() {
        *sample = (if index < 128 { 64_i8 } else { -64_i8 }) as u8;
    }
    bank
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_soundbank() -> SoundbankData {
        let path = Path::new("test-soundbank.wdb");
        let bytes = test_soundbank_wdb_bytes();
        let mut samples = Vec::with_capacity(bytes.len());
        samples.extend(bytes.into_iter().map(|sample| sample as i8));
        let bank = SoundbankData {
            samples: samples.into_boxed_slice(),
            drum_ranges: Box::new([]),
        };
        assert_eq!(bank.wavetable().len(), WAVETABLE_SAMPLES, "{path:?}");
        bank
    }

    #[test]
    fn renders_loops_fades_and_seeks() {
        let path = Path::new("synthetic.org");
        let mut decoder = Organya::from_parts(
            test_org_bytes(),
            test_soundbank(),
            path,
            44_100,
            2,
            Duration::from_secs(8),
        )
        .expect("open deterministic Organya fixture");

        assert_eq!(decoder.codec_name(), "Organya Org-02");
        assert_eq!(decoder.loop_points(), (0, 4));
        assert_eq!(decoder.duration(), Duration::from_millis(8_800));
        let mut pcm = vec![0.0_f32; 4_410 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render Organya"), 4_410);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        decoder
            .seek(Duration::from_millis(250))
            .expect("seek Organya");
        pcm.fill(0.0);
        assert_eq!(decoder.render(&mut pcm).expect("render after seek"), 4_410);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        decoder.seek(decoder.duration()).expect("seek to end");
        assert_eq!(decoder.render(&mut pcm).expect("render at end"), 0);
    }

    #[test]
    fn loads_orgorg_wdb_and_dumped_dat_pair() {
        let root = std::env::temp_dir().join(format!(
            "kog-organya-assets-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&root).expect("create asset fixture directory");
        let wdb = root.join("bank.wdb");
        std::fs::write(&wdb, test_soundbank_wdb_bytes()).expect("write WDB fixture");
        assert_eq!(load_wdb(&wdb).expect("load WDB").samples.len(), 25_600);

        std::fs::write(root.join("wavetable.dat"), test_soundbank_wdb_bytes())
            .expect("write wavetable fixture");
        std::fs::write(root.join("drums.dat"), vec![0_u8; 40_000]).expect("write drum fixture");
        let pair = load_dat_pair(&root).expect("load dat pair");
        assert_eq!(pair.samples.len(), 65_600);
        assert_eq!(pair.drum(0).map(<[i8]>::len), Some(5_000));
        assert!(pair.drum(1).is_none());

        std::fs::remove_file(wdb).ok();
        std::fs::remove_file(root.join("wavetable.dat")).ok();
        std::fs::remove_file(root.join("drums.dat")).ok();
        std::fs::remove_dir(root).ok();
    }
}
