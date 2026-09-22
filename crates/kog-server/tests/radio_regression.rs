//! End-to-end check that a full radio window builds against the real library
//! even with an unprovable pick in the round: the hanging miniusf is skipped
//! into the dead set instead of stalling the round. Needs the local music
//! library; skipped without it.
use std::path::PathBuf;

const LIBRARY_ROOT: &str = "/mnt/stuff/Music/Chiptune/VGM-Cartridge";
const ROUND_FILE: &str = "/home/ben/.config/kog/radio-round.json";

#[test]
fn window_builds_past_a_hanging_pick() {
    if !PathBuf::from(LIBRARY_ROOT).is_dir() || !PathBuf::from(ROUND_FILE).is_file() {
        eprintln!("skipped: real library or round file not present");
        return;
    }
    let save = PathBuf::from("/tmp/radio-regression-round.json");
    std::fs::copy(ROUND_FILE, &save).unwrap();
    let radio = kog_server::radio::Radio::new(Some(PathBuf::from(LIBRARY_ROOT)), Some(save), true);
    let started = std::time::Instant::now();
    let advance = radio.advance(None, None);
    let elapsed = started.elapsed();
    eprintln!(
        "advance took {elapsed:?} for {} entries (exhausted={})",
        advance.entries.len(),
        advance.exhausted
    );
    assert!(
        elapsed < std::time::Duration::from_secs(300),
        "a window must finish, took {elapsed:?}"
    );
    assert!(
        !advance.entries.is_empty(),
        "a window against a real library is never empty"
    );
}
