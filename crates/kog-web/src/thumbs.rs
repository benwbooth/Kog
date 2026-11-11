//! Album covers on the phone's song rows. A row on screen asks for its
//! song's cover once it has stayed there a moment (a fast scroll passes rows
//! by without asking), a few at a time. The server answers with the name of
//! a small thumbnail shared by every song of the album; the row then shows
//! it as an ordinary image, which the browser loads off the main thread and
//! keeps for good. Answers are remembered by song and by album, so the next
//! row of an album shows its cover at once. Until then, and for songs
//! without a cover, the row keeps its format icon.

use super::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Requests running at once.
const IN_FLIGHT: usize = 4;
/// How long a row stays on screen before it asks.
const SETTLE_MS: i32 = 150;
/// How soon to ask again while the server is still looking.
const RETRY_MS: i32 = 1200;
/// Answers remembered, before starting over.
const REMEMBERED: usize = 8192;
/// How long a row waits for its tags (which name its album) before asking
/// without them.
const TAGS_WAIT_MS: i32 = 4000;

struct Want {
    id: u64,
    url: String,
    /// The thumbnail's URL, around its name.
    image_base: String,
    image_query: String,
    song: String,
    album: Option<String>,
    shown: RwSignal<Option<String>>,
}

#[derive(Default)]
struct Loader {
    /// Song path to its thumbnail's URL (empty: no cover).
    songs: HashMap<String, String>,
    /// Folder and album tag to the album's thumbnail URL.
    albums: HashMap<String, String>,
    queue: VecDeque<Want>,
    wanted: HashSet<u64>,
    running: usize,
    next_id: u64,
}

thread_local! {
    static LOADER: RefCell<Loader> = RefCell::new(Loader::default());
}

fn album_key(song: &str, album: &str) -> String {
    let folder = song.rsplit_once('/').map(|(folder, _)| folder).unwrap_or_default();
    format!("{folder}\0{}", album.trim())
}

impl Loader {
    fn known(&self, song: &str, album: Option<&str>) -> Option<String> {
        self.songs.get(song).cloned().or_else(|| {
            album.and_then(|album| self.albums.get(&album_key(song, album)).cloned())
        })
    }

    fn remember(&mut self, song: &str, album: Option<&str>, url: &str) {
        if self.songs.len() >= REMEMBERED {
            self.songs.clear();
            self.albums.clear();
        }
        self.songs.insert(song.to_owned(), url.to_owned());
        if let Some(album) = album
            && !url.is_empty()
        {
            self.albums.insert(album_key(song, album), url.to_owned());
        }
    }
}

/// Whether song rows show as phone cards (where the covers are).
pub fn phone_rows() -> bool {
    web_sys::window()
        .and_then(|window| {
            window
                .match_media("(max-width: 820px), (pointer: coarse) and (max-width: 1200px)")
                .ok()
                .flatten()
        })
        .is_some_and(|query| query.matches())
}

fn show(shown: RwSignal<Option<String>>, url: &str) {
    if !url.is_empty() {
        let _ = shown.try_set(Some(url.to_owned()));
    }
}

/// Ask for `song`'s cover for a row; `shown` receives its image URL. Returns
/// the request to [`cancel`] when the row goes, or `None` when answered at
/// once.
fn request(
    base: &str,
    token: &str,
    kind: &str,
    song: &str,
    album: Option<String>,
    shown: RwSignal<Option<String>>,
) -> Option<u64> {
    if let Some(url) = LOADER.with(|loader| loader.borrow().known(song, album.as_deref())) {
        show(shown, &url);
        return None;
    }
    let url = format!(
        "{base}/api/art/thumb?kind={}&path={}&token={}{}",
        url_encode(kind),
        url_encode(song),
        url_encode(token),
        album
            .as_deref()
            .map(|album| format!("&album={}", url_encode(album)))
            .unwrap_or_default(),
    );
    let id = LOADER.with(|loader| {
        let mut loader = loader.borrow_mut();
        loader.next_id += 1;
        let id = loader.next_id;
        loader.wanted.insert(id);
        id
    });
    let want = Want {
        id,
        url,
        image_base: format!("{base}/api/art/thumbs/"),
        image_query: format!("?token={}", url_encode(token)),
        song: song.to_owned(),
        album,
        shown,
    };
    leptos::task::spawn_local(async move {
        let _ = sleep_ms(SETTLE_MS).await;
        enqueue(want);
    });
    Some(id)
}

/// The row went away: drop its request unless it is already running.
fn cancel(id: u64) {
    LOADER.with(|loader| {
        loader.borrow_mut().wanted.remove(&id);
    });
}

fn enqueue(want: Want) {
    LOADER.with(|loader| {
        let mut loader = loader.borrow_mut();
        if loader.wanted.contains(&want.id) {
            loader.queue.push_back(want);
        }
    });
    pump();
}

fn pump() {
    loop {
        let next = LOADER.with(|loader| {
            let mut loader = loader.borrow_mut();
            while loader.running < IN_FLIGHT {
                let want = loader.queue.pop_front()?;
                if !loader.wanted.contains(&want.id) {
                    continue;
                }
                // Another row of the album may have found it meanwhile.
                if let Some(url) = loader.known(&want.song, want.album.as_deref()) {
                    loader.wanted.remove(&want.id);
                    return Some(Err((want, url)));
                }
                loader.running += 1;
                return Some(Ok(want));
            }
            None
        });
        match next {
            None => return,
            Some(Err((want, url))) => show(want.shown, &url),
            Some(Ok(want)) => leptos::task::spawn_local(fetch(want)),
        }
    }
}

async fn fetch(want: Want) {
    let response = Request::get(&want.url)
        .header("X-Kog-Device", &device_id())
        .send()
        .await;
    let finished = |loader: &mut Loader| {
        loader.running = loader.running.saturating_sub(1);
    };
    let Ok(response) = response else {
        LOADER.with(|loader| finished(&mut loader.borrow_mut()));
        pump();
        return;
    };
    match response.status() {
        202 => {
            LOADER.with(|loader| finished(&mut loader.borrow_mut()));
            pump();
            let _ = sleep_ms(RETRY_MS).await;
            enqueue(want);
            return;
        }
        200 => {
            let image = response
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|body| body["image"].as_str().map(str::to_owned))
                .filter(|image| image.len() == 16);
            if let Some(image) = image {
                let url = format!("{}{image}{}", want.image_base, want.image_query);
                LOADER.with(|loader| {
                    loader
                        .borrow_mut()
                        .remember(&want.song, want.album.as_deref(), &url)
                });
                show(want.shown, &url);
            }
        }
        404 => LOADER.with(|loader| {
            loader
                .borrow_mut()
                .remember(&want.song, want.album.as_deref(), "")
        }),
        _ => {}
    }
    LOADER.with(|loader| {
        let mut loader = loader.borrow_mut();
        loader.wanted.remove(&want.id);
        finished(&mut loader);
    });
    pump();
}

/// Load the cover for a phone row of `entry` into `shown`, once the row's
/// tags arrive (they name its album, which the server finds without reading
/// the song) or after a short wait without them. Does nothing on the
/// desktop's table rows or for anything but songs.
pub(super) fn load_for_row(
    base: String,
    token: String,
    entry: &Entry,
    tags: Memo<Option<MetaRow>>,
    shown: RwSignal<Option<String>>,
) {
    if !matches!(entry.kind.as_str(), "local" | "archive") || !phone_rows() {
        return;
    }
    // 0: not asked yet; u64::MAX: answered; else the running request.
    let ticket = Arc::new(AtomicU64::new(0));
    let ask = {
        let ticket = ticket.clone();
        let kind = entry.kind.clone();
        let song = entry.path.clone();
        move |album: Option<String>| {
            if ticket.load(Ordering::Relaxed) != 0 {
                return;
            }
            let id = request(&base, &token, &kind, &song, album, shown).unwrap_or(u64::MAX);
            ticket.store(id, Ordering::Relaxed);
        }
    };
    let asked = {
        let ticket = ticket.clone();
        move || ticket.load(Ordering::Relaxed) != 0
    };
    {
        let ask = ask.clone();
        let asked = asked.clone();
        Effect::new(move |_| {
            // Stops listening once asked: nothing read, nothing tracked.
            if asked() {
                return;
            }
            if let Some(tags) = tags.get() {
                ask(Some(tags.album.unwrap_or_default()));
            }
        });
    }
    leptos::task::spawn_local(async move {
        let _ = sleep_ms(TAGS_WAIT_MS).await;
        // A row already gone has cancelled (and so "asked").
        if !asked() {
            ask(None);
        }
    });
    on_cleanup(move || {
        let id = ticket.swap(u64::MAX, Ordering::Relaxed);
        if id != 0 && id != u64::MAX {
            cancel(id);
        }
    });
}
