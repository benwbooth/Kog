//! Exercise SQLite's OS-level coordination, not just a process-local Mutex.
use kog_core::db::{LibraryDb, StoredEntry};
use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

#[test]
#[ignore = "subprocess helper for concurrent_processes_append_atomic_batches"]
fn database_worker() {
    let path = PathBuf::from(std::env::var_os("KOG_TEST_DATABASE").expect("database"));
    let worker = std::env::var("KOG_TEST_WORKER").unwrap();
    let db = LibraryDb::open_at(&path).unwrap();
    std::fs::write(path.with_extension(format!("ready-{worker}")), b"").unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !path.with_extension("start").exists() {
        assert!(Instant::now() < deadline, "parent did not start workers");
        std::thread::sleep(Duration::from_millis(5));
    }
    for batch in 0..20 {
        let entries = (0..3)
            .map(|item| StoredEntry {
                kind: "local".into(),
                path: format!("{worker}/{batch}/{item}"),
                entry: String::new(),
                fragment: None,
            })
            .collect::<Vec<_>>();
        db.append_entries(1, &entries).unwrap();
        let snapshot = db.playlist_entries(1).unwrap();
        assert_eq!(snapshot.len() % 3, 0, "observed a partially appended batch");
    }
}

#[test]
fn concurrent_processes_append_atomic_batches() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("kog.db");
    let db = LibraryDb::open_at(&path).unwrap();
    assert_eq!(db.create_playlist("Shared").unwrap(), 1);
    let mut workers = (0..4)
        .map(|worker| {
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "database_worker", "--ignored"])
                .env("KOG_TEST_DATABASE", &path)
                .env("KOG_TEST_WORKER", worker.to_string())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !(0..4).all(|worker| path.with_extension(format!("ready-{worker}")).exists()) {
        if Instant::now() > deadline {
            for child in &mut workers {
                let _ = child.kill();
                let _ = child.wait();
            }
            panic!("workers did not open their independent connections");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    std::fs::write(path.with_extension("start"), b"").unwrap();
    for child in workers {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let entries = db.playlist_entries(1).unwrap();
    assert_eq!(entries.len(), 4 * 20 * 3);
    let unique = entries
        .iter()
        .map(|entry| &entry.path)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), entries.len());
    for batch in entries.chunks_exact(3) {
        let prefix = batch[0].path.rsplit_once('/').unwrap().0;
        for (index, entry) in batch.iter().enumerate() {
            assert_eq!(entry.path, format!("{prefix}/{index}"));
        }
    }
}
