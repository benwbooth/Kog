//! Decoder integration: inspect the core producing PCM, never a second replay.
use super::*;
use crate::decoder::{DecoderSettings, PlaybackSource};
use crate::streaming::PcmReader;
use std::io::Read;
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    let tags = "title=Channel inspection fixture\nlength=0:03\nfade=0\n";
    match name {
        "mus" => crate::adlmidi::test_mus_bytes(),
        "wlf" => {
            let mut bytes = vec![0, 0, 0, 0];
            for (register, value, delay) in [
                (0x20, 1, 0u16),
                (0x23, 1, 0),
                (0x40, 0, 0),
                (0x43, 0, 0),
                (0x60, 0xf0, 0),
                (0x63, 0xf0, 0),
                (0x80, 0x0f, 0),
                (0x83, 0x0f, 0),
                (0xc0, 1, 0),
                (0xa0, 0x98, 0),
                (0xb0, 0x31, 2800),
                (0xb0, 0x11, 0),
            ] {
                bytes.extend_from_slice(&[register, value]);
                bytes.extend_from_slice(&delay.to_le_bytes());
            }
            bytes
        }
        "mod" => crate::openmpt::test_mod_bytes(),
        "nsf" => std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native/game-music-emu/test.nsf"),
        )
        .unwrap(),
        "cmf" => std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native/adplug/test/2.CMF"),
        )
        .unwrap(),
        "sid" => crate::sid::test_psid_bytes(false),
        "hvl" => crate::hively::test_multisubsong_hvl_bytes(),
        "org" => crate::organya::test_org_bytes(),
        "ncsf" => crate::ncsf::test_ncsf_bytes(Some(&crate::ncsf::test_sdat_bytes()), tags),
        "gsf" => crate::gsf::test_gsf_bytes(Some(&crate::gsf::test_gba_rom()), tags),
        "qsf" => crate::qsf::test_qsf_bytes(Some(&crate::qsf::test_qsf_program()), tags),
        "ssf" => crate::sdsf::test_sdsf_bytes(0x11, Some(&crate::sdsf::test_ssf_program()), tags),
        "dsf" => crate::sdsf::test_sdsf_bytes(0x12, Some(&crate::sdsf::test_dsf_program()), tags),
        "usf" => crate::usf::test_usf_bytes(Some(&crate::usf::test_usf_reserved()), tags),
        "psf" => crate::psf::test_psf_bytes(Some(&crate::psf::test_psf_executable()), tags),
        "psf2" => crate::psf::test_psf2_bytes(&[("psf2.irx", &crate::psf::test_psf2_irx())], tags),
        "snsf" => crate::psf::test_snsf_bytes(0, &crate::psf::test_snsf_rom(), &[], tags),
        "2sf" => crate::psf::test_twosf_bytes(0, &crate::psf::test_twosf_rom(), tags),
        "sfm" => crate::sfm::test_sfm_bytes(),
        "jxs" => crate::syntrax::test_jxs_bytes(),
        _ => panic!("unknown fixture {name}"),
    }
}

fn verify(name: &str, count: usize, pitched: bool) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(format!("fixture.{name}"));
    std::fs::write(&path, fixture(name)).unwrap();
    if name == "org" {
        std::fs::write(
            directory.path().join("soundbank.wdb"),
            crate::organya::test_soundbank_wdb_bytes(),
        )
        .unwrap();
    }
    let open = || {
        PcmReader::open(
            PlaybackSource {
                path: path.clone(),
                ..PlaybackSource::default()
            },
            DecoderSettings::default(),
        )
        .unwrap()
    };
    let (mut observed, mut plain) = if name == "sid" {
        // libsidplayfp seeds its power-on delay from time(0). Both independent
        // engines must have the same seed for a PCM invariance comparison.
        // Construction can cross a second while reSID's tables warm up.
        let second = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        };
        let mut pair = None;
        for _ in 0..4 {
            let before = second();
            let candidate = (open(), open());
            if second() == before {
                pair = Some(candidate);
                break;
            }
        }
        pair.expect("construct SID comparison engines within the same seed second")
    } else {
        (open(), open())
    };
    plain.inspection_monitor().set_enabled(false);
    let monitor = observed.inspection_monitor();
    let mut pcm = Vec::new();
    let mut latest = Snapshot::default();
    let mut saw_pitch = false;
    let blocks = (observed
        .duration()
        .unwrap_or(Duration::from_secs(2))
        .as_millis()
        / 20)
        .clamp(1, 100) as u64;
    let position = Duration::from_millis(blocks * 10);
    for i in 0..blocks {
        let mut block = [0u8; 3840]; // 10 ms at 48 kHz stereo float32
        observed.read_exact(&mut block).unwrap();
        pcm.extend_from_slice(&block);
        latest = monitor.snapshot(Duration::from_millis((i + 1) * 10), true, false);
        saw_pitch |= latest.channels.iter().any(|c| !c.notes.is_empty());
    }
    assert!(
        latest.channels.len() >= count,
        "{name}: expected at least {count} channels, got {}: {:?}",
        latest.channels.len(),
        latest.description
    );
    assert!(!pitched || saw_pitch, "{name}: no pitch observed");
    assert!(!latest.rows.is_empty(), "{name}: no tracker rows");
    let mut ids = latest.channels.iter().map(|c| c.id).collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids.len(),
        latest.channels.len(),
        "{name}: duplicate channel IDs"
    );
    assert!(
        pcm.chunks_exact(4)
            .any(|s| f32::from_le_bytes(s.try_into().unwrap()).abs() > 0.00001),
        "{name}: fixture produced no sound"
    );
    let paused = monitor.snapshot(position, false, false);
    assert_eq!(
        latest.channels, paused.channels,
        "{name}: pause changed channel state"
    );
    assert!(
        monitor
            .snapshot(Duration::from_secs(1), true, true)
            .channels
            .is_empty()
    );
    // Disabling the feed also restores each source's ordinary render size.
    let mut baseline = vec![0; pcm.len()];
    plain.read_exact(&mut baseline).unwrap();
    let maximum_error = pcm
        .chunks_exact(4)
        .zip(baseline.chunks_exact(4))
        .map(|(a, b)| {
            (f32::from_le_bytes(a.try_into().unwrap()) - f32::from_le_bytes(b.try_into().unwrap()))
                .abs()
        })
        .fold(0f32, f32::max);
    assert!(
        maximum_error < 0.003,
        "{name}: inspection changed PCM (max error {maximum_error})"
    );
    let sought = position / 2;
    observed.seek(sought).unwrap();
    let mut block = [0u8; 38400];
    observed.read_exact(&mut block).unwrap();
    let after = monitor.snapshot(sought + Duration::from_millis(100), true, false);
    assert!(
        after.channels.len() >= count,
        "{name}: no channels after seek"
    );
    if let Some(destination) = std::env::var_os("KOG_INSPECTION_FIXTURES") {
        let destination = Path::new(&destination);
        std::fs::create_dir_all(destination).unwrap();
        std::fs::copy(&path, destination.join(format!("fixture.{name}"))).unwrap();
        std::fs::write(
            destination.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&latest).unwrap(),
        )
        .unwrap();
        if name == "org" {
            std::fs::copy(
                directory.path().join("soundbank.wdb"),
                destination.join("soundbank.wdb"),
            )
            .unwrap();
        }
    }
    eprintln!(
        "{name}: {} channels, pitch={saw_pitch}, PCM max error={maximum_error}",
        latest.channels.len()
    );
}

macro_rules! decoder_test {
    ($test:ident,$ext:literal,$count:literal,$pitch:literal) => {
        #[test]
        fn $test() {
            verify($ext, $count, $pitch);
        }
    };
}
decoder_test!(inspection_mod, "mod", 4, true);
decoder_test!(inspection_mus, "mus", 16, true);
decoder_test!(inspection_nsf, "nsf", 5, true);
decoder_test!(inspection_cmf, "cmf", 9, false); // Official fixture begins with OPL drums.
decoder_test!(inspection_opl_tone, "wlf", 9, true);
decoder_test!(inspection_sid, "sid", 3, true);
decoder_test!(inspection_hvl, "hvl", 4, true);
decoder_test!(inspection_org, "org", 16, true);
decoder_test!(inspection_ncsf, "ncsf", 16, true);
decoder_test!(inspection_gsf, "gsf", 6, true);
decoder_test!(inspection_qsf, "qsf", 19, false);
decoder_test!(inspection_ssf, "ssf", 32, false);
decoder_test!(inspection_dsf, "dsf", 64, false);
decoder_test!(inspection_usf, "usf", 2, false);
#[cfg(not(windows))]
decoder_test!(inspection_psf, "psf", 24, false);
#[cfg(not(windows))]
decoder_test!(inspection_psf2, "psf2", 48, false);
#[cfg(not(windows))]
decoder_test!(inspection_snsf, "snsf", 8, false);
#[cfg(not(windows))]
decoder_test!(inspection_twosf, "2sf", 16, false);
decoder_test!(inspection_sfm, "sfm", 8, false);
decoder_test!(inspection_syntrax, "jxs", 1, true);
