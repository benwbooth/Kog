//! Count tracker rows per second: `cargo run --example tracker_rate -- song.nsf`
use std::io::Read;
fn main() {
    let path = std::env::args().nth(1).expect("song path");
    let mut pcm = kog_audio::streaming::PcmReader::open_path(
        path.into(),
        kog_audio::decoder::DecoderSettings::new(None, kog_audio::settings::MidiEngine::Opl3Windows),
    )
    .unwrap();
    let monitor = pcm.inspection_monitor();
    let frame_bytes = 4 * u64::from(pcm.channels());
    let rate = f64::from(pcm.sample_rate());
    let mut buffer = vec![0u8; 16 * 1024];
    let mut consumed = 0u64;
    let mut times = std::collections::BTreeSet::new();
    let mut sample = Vec::new();
    while (consumed / frame_bytes) as f64 / rate < 30.0 {
        let count = pcm.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        consumed += count as u64;
        let seconds = (consumed / frame_bytes) as f64 / rate;
        let snapshot = monitor.snapshot(std::time::Duration::from_secs_f64(seconds), true, false);
        for row in &snapshot.rows {
            if times.insert(row.time.to_bits()) && sample.len() < 12 {
                sample.push(format!("{:.3} {}", row.time, row.cells.iter().map(|c| format!("{}:{}", c.channel, c.notes)).collect::<Vec<_>>().join(",")));
            }
        }
    }
    println!("{} rows in 30 s ({:.1}/s)", times.len(), times.len() as f64 / 30.0);
    for line in sample {
        println!("{line}");
    }
}
