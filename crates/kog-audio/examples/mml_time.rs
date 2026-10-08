//! Time a whole-song MML recording: `cargo run --release --example mml_time -- song.nsf`
fn main() {
    let path = std::env::args().nth(1).expect("song path");
    let started = std::time::Instant::now();
    let progress = kog_audio::inspection::score::Progress::default();
    let mut pcm = kog_audio::streaming::PcmReader::open_path(
        path.into(),
        // MIDI needs a synth that loads nothing extra; scores do not depend on it.
        kog_audio::decoder::DecoderSettings::new(None, kog_audio::settings::MidiEngine::Opl3Windows),
    ).unwrap();
    let opened = started.elapsed();
    let seconds = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(600.0);
    let score = kog_audio::inspection::score::record(&mut pcm, "t", &progress, seconds, &mut |_| {}).unwrap();
    let recorded = started.elapsed();
    let document = kog_audio::inspection::mml::encode(&score);
    let parsed = kog_audio::inspection::mml::parse(&document.text).unwrap();
    assert!(parsed == score, "the MML text does not read back as the recorded score");
    if let Some(path) = std::env::var_os("KOG_MML_OUT") {
        std::fs::write(path, &document.text).unwrap();
    }
    println!(
        "open {:.2?}, record {:.2?} for {:.1}s of audio, encode {:.2?}, {} bytes",
        opened,
        recorded - opened,
        progress.recorded_ms.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0,
        started.elapsed() - recorded,
        document.text.len()
    );
}
