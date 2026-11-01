//! `search_bench <folder> <query> [threads]`: how long a whole search takes.
use std::sync::atomic::{AtomicU64, Ordering};

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args.next().expect("folder");
    let matcher = kog_core::file_search::Matcher::new(&args.next().expect("query"));
    let threads = args.next().map_or(12, |count| count.parse().expect("threads"));
    let (scanned, found) = (AtomicU64::new(0), AtomicU64::new(0));
    let start = std::time::Instant::now();
    kog_core::file_search::walk_with_threads(threads, root.into(), (), |folder, (), subfolders| {
        let mut scratch = Vec::new();
        for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
            let name = entry.file_name();
            scanned.fetch_add(1, Ordering::Relaxed);
            if matcher.is_match(name.as_encoded_bytes(), &mut scratch) {
                found.fetch_add(1, Ordering::Relaxed);
            }
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                subfolders.push(entry.path(), ());
            }
        }
        true
    });
    println!("{threads} threads: {:?}, {scanned:?} names, {found:?} matches", start.elapsed());
}
