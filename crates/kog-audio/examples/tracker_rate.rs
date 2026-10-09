//! Count tracker rows per second: `cargo run --example tracker_rate -- song.nsf [subsong]`
use std::io::Read;
fn main() {
    let path = std::env::args().nth(1).expect("song path");
    let subsong = std::env::args().nth(2).and_then(|s| s.parse().ok());
    let mut pcm = kog_audio::streaming::PcmReader::open_path_subsong(
        path.into(),
        subsong,
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
            // KOG_CHANNEL=<id> lists that channel's cells instead.
            let only: Option<u32> = std::env::var("KOG_CHANNEL").ok().and_then(|id| id.parse().ok());
            let cells: Vec<_> = row.cells.iter().filter(|c| only.is_none_or(|id| c.channel == id)).collect();
            if times.insert(row.time.to_bits()) && sample.len() < 24 && !cells.is_empty() {
                sample.push(format!("{:.3} {}", row.time, cells.iter().map(|c| format!("{}:{} {}", c.channel, c.notes, c.instrument)).collect::<Vec<_>>().join(",")));
            }
        }
    }
    println!("{} rows in 30 s ({:.1}/s)", times.len(), times.len() as f64 / 30.0);
    for line in sample {
        println!("{line}");
    }
}
