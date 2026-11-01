//! The file search's two hot paths, the way ripgrep and fd do them: folders are
//! walked by a pool of threads that steal each other's queued folders, and
//! names are matched with a SIMD substring search over reusable buffers.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crossbeam_deque::{Injector, Steal, Stealer, Worker};
use memchr::memmem::Finder;
use unicode_normalization::UnicodeNormalization;

/// More threads than this only make a disk seek harder (ripgrep's cap too).
const MAX_THREADS: usize = 12;

/// Every whitespace-separated word of a query, ready to look for in names.
pub struct Matcher {
    words: Vec<Finder<'static>>,
}

impl Matcher {
    pub fn new(query: &str) -> Self {
        let words = fold(query)
            .split_whitespace()
            .map(|word| Finder::new(word.as_bytes()).into_owned())
            .collect();
        Self { words }
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Whether `name` holds every word. `scratch` is reused across calls so
    /// matching a name allocates nothing; each thread keeps its own.
    pub fn is_match(&self, name: &[u8], scratch: &mut Vec<u8>) -> bool {
        scratch.clear();
        if name.is_ascii() {
            scratch.extend_from_slice(name);
            scratch.make_ascii_lowercase();
        } else {
            scratch.extend_from_slice(fold(&String::from_utf8_lossy(name)).as_bytes());
        }
        self.words.iter().all(|word| word.find(scratch).is_some())
    }
}

/// Compatibility normalized and lowercased, so "ＦＦ" finds "ff".
fn fold(text: &str) -> String {
    text.nfkc().collect::<String>().to_lowercase()
}

/// Hands a folder's subfolders back to the walk.
pub struct Subfolders<'a, T> {
    local: &'a Worker<(PathBuf, T)>,
    queued: &'a AtomicUsize,
}

impl<T> Subfolders<'_, T> {
    pub fn push(&self, path: PathBuf, state: T) {
        self.queued.fetch_add(1, Ordering::SeqCst);
        self.local.push((path, state));
    }
}

/// Run `visit` on `root` and every folder it pushes, across a thread pool.
/// Each thread works depth first through its own queue, which keeps a folder's
/// neighbors together on disk, and idle threads steal the oldest (largest)
/// folders from the others. `T` travels with a folder, such as whether a
/// parent matched. The walk stops early when `visit` returns false.
pub fn walk<T, F>(root: PathBuf, state: T, visit: F)
where
    T: Send,
    F: Fn(&Path, T, &Subfolders<'_, T>) -> bool + Sync,
{
    let threads = std::thread::available_parallelism()
        .map_or(4, |count| count.get())
        .min(MAX_THREADS);
    walk_with_threads(threads, root, state, visit);
}

pub fn walk_with_threads<T, F>(threads: usize, root: PathBuf, state: T, visit: F)
where
    T: Send,
    F: Fn(&Path, T, &Subfolders<'_, T>) -> bool + Sync,
{
    let injector = Injector::new();
    injector.push((root, state));
    // Folders queued or being visited; the walk is over at zero.
    let queued = AtomicUsize::new(1);
    let stop = AtomicBool::new(false);
    let locals: Vec<Worker<(PathBuf, T)>> = (0..threads.max(1)).map(|_| Worker::new_lifo()).collect();
    let stealers: Vec<Stealer<(PathBuf, T)>> = locals.iter().map(Worker::stealer).collect();
    std::thread::scope(|scope| {
        for local in locals {
            let (injector, stealers, queued, stop, visit) = (&injector, &stealers, &queued, &stop, &visit);
            scope.spawn(move || {
                let subfolders = Subfolders { local: &local, queued };
                while !stop.load(Ordering::Relaxed) {
                    match next_folder(&local, injector, stealers) {
                        Some((path, state)) => {
                            if !visit(&path, state, &subfolders) {
                                stop.store(true, Ordering::Relaxed);
                            }
                            queued.fetch_sub(1, Ordering::SeqCst);
                        }
                        None if queued.load(Ordering::SeqCst) == 0 => break,
                        None => std::thread::yield_now(),
                    }
                }
            });
        }
    });
}

fn next_folder<T>(local: &Worker<T>, injector: &Injector<T>, stealers: &[Stealer<T>]) -> Option<T> {
    local.pop().or_else(|| {
        std::iter::repeat_with(|| {
            injector
                .steal_batch_and_pop(local)
                .or_else(|| stealers.iter().map(Stealer::steal).collect())
        })
        .find(|steal| !steal.is_retry())
        .and_then(Steal::success)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn every_word_matches_anywhere_ignoring_case_and_width() {
        let matcher = Matcher::new("  WICKED child ");
        let mut scratch = Vec::new();
        assert!(matcher.is_match(b"04 Wicked Child.vgz", &mut scratch));
        assert!(matcher.is_match(b"Child of the WICKED.nsf", &mut scratch));
        assert!(!matcher.is_match(b"Wicked.nsf", &mut scratch));
        assert!(Matcher::new("ff").is_match("ＦＦ７ ドラクエ.spc".as_bytes(), &mut scratch));
        assert!(Matcher::new("ドラクエ").is_match("ＦＦ７ ドラクエ.spc".as_bytes(), &mut scratch));
        assert!(Matcher::new("  ").is_empty());
    }

    #[test]
    fn the_walk_reaches_every_folder_once_and_carries_state() {
        let root = tempfile::tempdir().unwrap();
        for one in 0..20 {
            for two in 0..20 {
                std::fs::create_dir_all(root.path().join(format!("{one}/{two}/leaf"))).unwrap();
            }
        }
        for threads in [1, 8] {
            let seen = Mutex::new(Vec::new());
            walk_with_threads(threads, root.path().to_owned(), 0_usize, |folder, depth, subfolders| {
                seen.lock().unwrap().push((folder.to_owned(), depth));
                for entry in std::fs::read_dir(folder).unwrap() {
                    subfolders.push(entry.unwrap().path(), depth + 1);
                }
                true
            });
            let mut seen = seen.into_inner().unwrap();
            assert_eq!(seen.len(), 1 + 20 + 400 + 400);
            assert_eq!(seen.iter().filter(|(_, depth)| *depth == 3).count(), 400);
            seen.sort();
            seen.dedup();
            assert_eq!(seen.len(), 1 + 20 + 400 + 400);
        }
    }

    #[test]
    fn the_walk_stops_when_told_to() {
        let root = tempfile::tempdir().unwrap();
        for one in 0..50 {
            std::fs::create_dir_all(root.path().join(one.to_string())).unwrap();
        }
        let visits = AtomicUsize::new(0);
        walk_with_threads(1, root.path().to_owned(), (), |folder, (), subfolders| {
            for entry in std::fs::read_dir(folder).unwrap() {
                subfolders.push(entry.unwrap().path(), ());
            }
            visits.fetch_add(1, Ordering::Relaxed) < 2
        });
        assert_eq!(visits.load(Ordering::Relaxed), 3);
    }
}
