//! The Home tab's mixes, made from the listening history, the stars and
//! the library index.
//!
//! Taste comes from plays: a song heard to the end counts most, one heard
//! for half a minute counts some, one skipped in the first seconds counts
//! against it, and every play fades with age. Starred songs count as
//! several plays. Mixes then bring new songs from every source in turns
//! (the same composers, the rest of an album, songs that sound alike, the
//! songs beside a played one, new files), never letting one album or one
//! composer take a mix over. Songs are only grouped and named by their
//! tags, never by folder names.
//!
//! Daily mixes are made once a local day and Discover Weekly once a week;
//! a saved Home is kept until it is due, whichever Kog made it.

use std::collections::{HashMap, HashSet};

use kog_core::db::{PlayRecord, StoredEntry};
use serde::{Deserialize, Serialize};

use super::features;
use super::store::{Song, Store};

pub const DAY_MS: i64 = 86_400_000;

/// Songs in a Daily Mix and in Discover Weekly.
const DAILY_LENGTH: usize = 40;
const WEEKLY_LENGTH: usize = 30;
const MAX_DAILY_MIXES: usize = 6;
/// A song heard at least this long counts as heard.
const HEARD_MS: i64 = 30_000;
/// A song left before this counts as skipped.
const SKIPPED_MS: i64 = 12_000;
/// How fast plays fade, in days.
const MEMORY_DAYS: f64 = 60.0;
/// How many plays a star counts as.
const STAR_WEIGHT: f64 = 3.0;

/// One song in a mix, shaped like every other API entry so all frontends
/// can show and queue it with their existing row code.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MixSong {
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub entry: String,
    #[serde(default)]
    pub fragment: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub composer: String,
    #[serde(default)]
    pub year: Option<i64>,
    #[serde(default)]
    pub system: String,
    /// Milliseconds, as the queue's metadata expects.
    #[serde(default)]
    pub duration: i64,
}

impl MixSong {
    fn from_song(song: &Song) -> Self {
        Self {
            kind: song.kind.clone(),
            path: song.path.clone(),
            entry: song.entry.clone(),
            fragment: song.fragment.clone(),
            title: song.title.clone(),
            artist: if song.artist.is_empty() { song.credit().to_owned() } else { song.artist.clone() },
            album: song.album.clone(),
            composer: song.composer.clone(),
            year: song.year,
            system: song.system.clone(),
            duration: song.duration_ms,
        }
    }

    pub fn stored_entry(&self) -> StoredEntry {
        StoredEntry {
            kind: self.kind.clone(),
            path: self.path.clone(),
            entry: self.entry.clone(),
            fragment: self.fragment.clone(),
        }
    }

    /// The entry as the queue and playlist tabs take it.
    pub fn entry_json(&self) -> serde_json::Value {
        let mut value = serde_json::json!({
            "kind": self.kind,
            "path": self.path,
            "entry": self.entry,
            "title": self.title,
            "artist": self.artist,
            "album": self.album,
            "composer": self.composer,
            "codec": self.system,
        });
        if let Some(fragment) = &self.fragment {
            value["fragment"] = fragment.clone().into();
        }
        if let Some(year) = self.year {
            value["year"] = year.into();
        }
        if self.duration > 0 {
            value["duration"] = self.duration.into();
        }
        value
    }
}

/// The format or chip a decoder reports, without the decoder's own name
/// ("Nintendo DS Sound Format (NCSF) / SSEQPlayer" is shown as its first
/// part).
pub fn system_name(codec: &str) -> String {
    codec.split(" / ").next().unwrap_or("").trim().to_owned()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mix {
    /// Stable for the slot ("daily-1", "weekly"), so a tab that shows a mix
    /// shows the new one once it is made.
    pub id: String,
    /// daily, weekly, album, artist, rediscover, year, new.
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub songs: Vec<MixSong>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub title: String,
    pub mixes: Vec<Mix>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Home {
    /// The local day (days since the epoch, local midnight) the daily mixes
    /// were made for, and the week (days since the epoch of its Monday) for
    /// Discover Weekly.
    pub day: i64,
    pub week: i64,
    pub made_at: i64,
    pub sections: Vec<Section>,
    /// Made from too little to go on (a new library index or history): made
    /// again once the index has grown, instead of waiting a day.
    #[serde(default)]
    pub thin: bool,
    /// Songs in the index when it was made.
    #[serde(default)]
    pub indexed: i64,
}

impl Home {
    pub fn mix(&self, id: &str) -> Option<&Mix> {
        self.sections.iter().flat_map(|section| &section.mixes).find(|mix| mix.id == id)
    }

    pub fn weekly(&self) -> Option<&Mix> {
        self.mix("weekly")
    }
}

/// A small, seedable random source, so a day's mixes come out the same
/// wherever they are made from the same data.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn below(&mut self, bound: usize) -> usize {
        if bound == 0 { 0 } else { (self.next() % bound as u64) as usize }
    }
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            let other = self.below(index + 1);
            items.swap(index, other);
        }
    }
}

fn fold(text: &str) -> String {
    text.trim().to_lowercase()
}

/// The album a song belongs to: its album tag with the album artist, as the
/// cover grid groups them. Songs without an album tag belong to none.
fn album_key(album: &str, album_artist: &str) -> Option<String> {
    let album = fold(album);
    (!album.is_empty()).then(|| format!("{album}\u{1f}{}", fold(album_artist)))
}

fn song_album_key(song: &Song) -> Option<String> {
    album_key(&song.album, &song.album_artist)
}

/// Which songs share a locator family: a file's song and its "#0" are one.
fn identity(locator: &str) -> String {
    locator.strip_suffix("#0").unwrap_or(locator).to_owned()
}

/// What the history says about the listener.
#[derive(Default)]
pub struct Taste {
    /// Score per song identity (see [`identity`]): positive is liked.
    pub songs: HashMap<String, f64>,
    /// The best play record of each scored song, for its tags and entry.
    pub records: HashMap<String, PlayRecord>,
    /// Every song heard or starred, so mixes bring new ones.
    pub heard: HashSet<String>,
    /// Songs heard in the last two days, kept out of the mixes.
    pub recent: HashSet<String>,
    /// When each song was last heard.
    pub last_heard: HashMap<String, i64>,
    /// Liked credits (composer or artist), folded, by score.
    pub credits: Vec<(String, String, f64)>,
    pub albums: HashMap<String, f64>,
    pub systems: HashMap<String, f64>,
    pub years: HashMap<i64, f64>,
}

impl Taste {
    pub fn from_history(plays: &[PlayRecord], stars: &[StoredEntry], now: i64) -> Self {
        let mut taste = Taste::default();
        let mut credits: HashMap<String, (String, f64)> = HashMap::new();
        for play in plays {
            let key = identity(&play.locator);
            let age_days = (now - play.played_at).max(0) as f64 / DAY_MS as f64;
            let fade = (-age_days / MEMORY_DAYS).exp();
            let heard = play.finished
                || play.listened_ms >= HEARD_MS
                || (play.length_ms > 0 && play.listened_ms * 2 >= play.length_ms);
            let weight = if play.finished {
                1.0
            } else if heard {
                0.7
            } else if play.listened_ms < SKIPPED_MS {
                -0.6
            } else {
                0.2
            };
            if heard {
                taste.heard.insert(key.clone());
                let last = taste.last_heard.entry(key.clone()).or_insert(0);
                *last = (*last).max(play.played_at);
                if now - play.played_at < 2 * DAY_MS {
                    taste.recent.insert(key.clone());
                }
            }
            *taste.songs.entry(key.clone()).or_insert(0.0) += weight * fade;
            let better = taste
                .records
                .get(&key)
                .is_none_or(|record| record.title.is_empty() && !play.title.is_empty());
            if better {
                taste.records.insert(key, play.clone());
            }
        }
        for star in stars {
            let Ok(locator) = crate::media_filter::locator_for(star) else { continue };
            let key = identity(&locator);
            taste.heard.insert(key.clone());
            *taste.songs.entry(key).or_insert(0.0) += STAR_WEIGHT;
        }
        for (key, score) in &taste.songs {
            let Some(record) = taste.records.get(key) else { continue };
            let credit = [&record.composer, &record.artist, &record.album_artist]
                .into_iter()
                .map(|value| value.trim())
                .find(|value| !value.is_empty())
                .unwrap_or("");
            if !credit.is_empty() {
                let entry = credits.entry(fold(credit)).or_insert_with(|| (credit.to_owned(), 0.0));
                entry.1 += score;
            }
            if let Some(album) = album_key(&record.album, &record.album_artist) {
                *taste.albums.entry(album).or_insert(0.0) += score;
            }
            let system = system_name(&record.codec);
            if !system.is_empty() {
                *taste.systems.entry(system).or_insert(0.0) += score;
            }
            if let Some(year) = record.year {
                *taste.years.entry(year).or_insert(0.0) += score;
            }
        }
        taste.credits = credits
            .into_iter()
            .map(|(key, (name, score))| (key, name, score))
            .filter(|(_, _, score)| *score > 0.0)
            .collect();
        taste
            .credits
            .sort_by(|a, b| b.2.total_cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
        taste
    }

    fn liked(&self) -> Vec<(&String, f64)> {
        let mut liked: Vec<(&String, f64)> = self
            .songs
            .iter()
            .filter(|(_, score)| **score > 0.3)
            .map(|(key, score)| (key, *score))
            .collect();
        liked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        liked
    }

    fn disliked(&self, locator: &str) -> bool {
        self.songs.get(&identity(locator)).is_some_and(|score| *score < -0.5)
    }

    fn is_heard(&self, locator: &str) -> bool {
        self.heard.contains(&identity(locator))
    }

    fn credit_score(&self, credit: &str) -> f64 {
        let credit = fold(credit);
        self.credits
            .iter()
            .find(|(key, ..)| *key == credit)
            .map_or(0.0, |(.., score)| *score)
    }
}

/// Songs taken from several sources in turns, each source offering its own
/// best first, with caps per album and per credit so no one of them takes
/// the mix over.
struct Blend {
    songs: Vec<Song>,
    seen: HashSet<String>,
    albums: HashMap<String, usize>,
    credits: HashMap<String, usize>,
    album_cap: usize,
    credit_cap: usize,
}

impl Blend {
    fn new(album_cap: usize, credit_cap: usize) -> Self {
        Self {
            songs: Vec::new(),
            seen: HashSet::new(),
            albums: HashMap::new(),
            credits: HashMap::new(),
            album_cap,
            credit_cap,
        }
    }

    fn exclude(&mut self, locator: &str) {
        self.seen.insert(identity(locator));
    }

    fn try_add(&mut self, song: &Song) -> bool {
        let key = identity(&song.locator);
        if self.seen.contains(&key) {
            return false;
        }
        let album = song_album_key(song);
        if let Some(album) = &album
            && self.albums.get(album).copied().unwrap_or(0) >= self.album_cap
        {
            return false;
        }
        let credit = fold(song.credit());
        if !credit.is_empty() && self.credits.get(&credit).copied().unwrap_or(0) >= self.credit_cap {
            return false;
        }
        self.seen.insert(key);
        if let Some(album) = album {
            *self.albums.entry(album).or_insert(0) += 1;
        }
        if !credit.is_empty() {
            *self.credits.entry(credit).or_insert(0) += 1;
        }
        self.songs.push(song.clone());
        true
    }

    /// Take from `sources` in turns until `length` songs are in or every
    /// source has run dry.
    fn round_robin(&mut self, mut sources: Vec<Vec<Song>>, length: usize) {
        let mut cursors = vec![0usize; sources.len()];
        loop {
            let mut progressed = false;
            for (index, source) in sources.iter_mut().enumerate() {
                if self.songs.len() >= length {
                    return;
                }
                while cursors[index] < source.len() {
                    let song = &source[cursors[index]];
                    cursors[index] += 1;
                    if self.try_add(song) {
                        progressed = true;
                        break;
                    }
                }
            }
            if !progressed {
                return;
            }
        }
    }
}

/// The song a play record stands for, from the index when it has it
/// (with its sound features), else from the record's own tags.
fn record_song(store: &Store, record: &PlayRecord) -> Song {
    store.song(&record.locator).unwrap_or_else(|| Song {
        locator: record.locator.clone(),
        file_key: record.locator.split('#').next().unwrap_or("").to_owned(),
        kind: record.kind.clone(),
        path: record.path.clone(),
        entry: record.entry.clone(),
        fragment: record.fragment.clone(),
        title: record.title.clone(),
        artist: record.artist.clone(),
        album: record.album.clone(),
        album_artist: record.album_artist.clone(),
        composer: record.composer.clone(),
        genre: record.genre.clone(),
        year: record.year,
        system: system_name(&record.codec),
        duration_ms: record.length_ms,
        ..Song::default()
    })
}

/// Songs closest in sound to `target`, best first.
fn nearest(target: &[f32], candidates: &[Song], limit: usize) -> Vec<Song> {
    let mut scored: Vec<(f32, &Song)> = candidates
        .iter()
        .filter_map(|song| {
            let features = song.features.as_ref()?;
            Some((features::distance(target, features), song))
        })
        .collect();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));
    scored.into_iter().take(limit).map(|(_, song)| song.clone()).collect()
}

/// A run of songs from a random place in the index.
fn random_sample(store: &Store, rng: &mut Rng, limit: usize, with_features: bool) -> Vec<Song> {
    let max = store.max_id();
    if max == 0 {
        return Vec::new();
    }
    let mut songs = Vec::new();
    // A few short runs from different places mix the library better than
    // one long run from one folder.
    let runs = 8;
    for _ in 0..runs {
        let start = 1 + (rng.next() % max as u64) as i64;
        let take = limit / runs + 1;
        if with_features {
            songs.extend(store.sample_with_features(start, take));
        } else {
            songs.extend(store.sample(start, take));
        }
    }
    rng.shuffle(&mut songs);
    songs
}

fn subtitle_for(songs: &[Song]) -> String {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for song in songs {
        let credit = song.credit();
        if credit.is_empty() {
            continue;
        }
        match counts.iter_mut().find(|(name, _)| name.eq_ignore_ascii_case(credit)) {
            Some((_, count)) => *count += 1,
            None => counts.push((credit.to_owned(), 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    let names: Vec<String> = counts.iter().take(3).map(|(name, _)| name.clone()).collect();
    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        _ => format!("{} and more", names.join(", ")),
    }
}

/// A subtitle that names the songs' composers after `lead`, when they
/// have any.
fn with_names(lead: &str, songs: &[Song]) -> String {
    match subtitle_for(songs) {
        names if names.is_empty() => lead.to_owned(),
        names => format!("{lead} · {names}"),
    }
}

/// The system most of the songs share, when one does.
fn common_system(songs: &[Song]) -> Option<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for song in songs {
        if !song.system.is_empty() {
            *counts.entry(song.system.as_str()).or_insert(0) += 1;
        }
    }
    let (system, count) = counts.into_iter().max_by_key(|(_, count)| *count)?;
    (count * 3 >= songs.len() * 2).then(|| system.to_owned())
}

fn mix_from(id: String, kind: &str, title: String, subtitle: String, songs: &[Song]) -> Mix {
    Mix {
        id,
        kind: kind.to_owned(),
        title,
        subtitle,
        songs: songs.iter().map(MixSong::from_song).collect(),
    }
}

/// What a liked song's mix draws its new songs from: its composer's other
/// songs, the rest of its album, the songs beside it in the library, and
/// songs that sound like it.
fn new_songs_for(store: &Store, seeds: &[Song], sample: &[Song], taste: &Taste, rng: &mut Rng) -> Vec<Vec<Song>> {
    let fresh = |songs: Vec<Song>| -> Vec<Song> {
        songs
            .into_iter()
            .filter(|song| !taste.is_heard(&song.locator) && !taste.disliked(&song.locator))
            .collect()
    };
    let mut by_credit = Vec::new();
    let mut by_album = Vec::new();
    let mut beside = Vec::new();
    let mut credits_done = HashSet::new();
    let mut albums_done = HashSet::new();
    for seed in seeds.iter().take(12) {
        let credit = fold(seed.credit());
        if !credit.is_empty() && credits_done.insert(credit) {
            by_credit.extend(fresh(store.songs_by_credit(seed.credit(), 200)));
        }
        if !seed.album.is_empty() && albums_done.insert(fold(&seed.album)) {
            let album = song_album_key(seed);
            by_album.extend(fresh(
                store
                    .songs_by_album(&seed.album, 200)
                    .into_iter()
                    .filter(|song| song_album_key(song) == album)
                    .collect(),
            ));
        }
        if !seed.folder.is_empty() {
            beside.extend(fresh(store.songs_in_folder(&seed.folder, 300)));
        }
    }
    rng.shuffle(&mut by_credit);
    rng.shuffle(&mut by_album);
    rng.shuffle(&mut beside);
    let mut sounds_like = Vec::new();
    let vectors: Vec<&[f32]> = seeds.iter().filter_map(|seed| seed.features.as_deref()).collect();
    if let Some(center) = features::centroid(vectors) {
        let system = common_system(seeds);
        let mut pool: Vec<Song> = fresh(sample.to_vec());
        if let Some(system) = &system {
            let max = store.max_id().max(1);
            let start = 1 + (rng.next() % max as u64) as i64;
            pool.extend(fresh(store.songs_of_system(system, start, 600)));
        }
        sounds_like = nearest(&center, &pool, 120);
    }
    vec![sounds_like, by_credit, by_album, beside]
}

/// Group liked songs into up to [`MAX_DAILY_MIXES`] styles: songs by the
/// same composer or on the same system that sound alike go together.
fn clusters(liked: &[Song], rng: &mut Rng) -> Vec<Vec<Song>> {
    if liked.is_empty() {
        return Vec::new();
    }
    let count = (liked.len() / 10).clamp(1, MAX_DAILY_MIXES);
    let apart = |a: &Song, b: &Song| -> f32 {
        let mut distance = match (&a.features, &b.features) {
            (Some(x), Some(y)) => features::distance(x, y),
            _ => 1.0,
        };
        if !a.credit().is_empty() && a.credit().eq_ignore_ascii_case(b.credit()) {
            distance *= 0.4;
        }
        if !a.system.is_empty() && a.system == b.system {
            distance *= 0.7;
        } else {
            distance += 0.5;
        }
        distance
    };
    // Farthest-first centres from the best liked song, then each song joins
    // its nearest centre.
    let mut centres: Vec<usize> = vec![0];
    while centres.len() < count {
        let next = (0..liked.len())
            .filter(|index| !centres.contains(index))
            .max_by(|a, b| {
                let da = centres.iter().map(|c| apart(&liked[*a], &liked[*c])).fold(f32::MAX, f32::min);
                let db = centres.iter().map(|c| apart(&liked[*b], &liked[*c])).fold(f32::MAX, f32::min);
                da.total_cmp(&db)
            });
        match next {
            Some(index) => centres.push(index),
            None => break,
        }
    }
    let mut groups: Vec<Vec<Song>> = vec![Vec::new(); centres.len()];
    for song in liked {
        let best = centres
            .iter()
            .enumerate()
            .min_by(|a, b| apart(song, &liked[*a.1]).total_cmp(&apart(song, &liked[*b.1])))
            .map(|(index, _)| index)
            .unwrap_or(0);
        groups[best].push(song.clone());
    }
    groups.retain(|group| group.len() >= 3);
    for group in &mut groups {
        let head = group.remove(0);
        rng.shuffle(group);
        group.insert(0, head);
    }
    groups
}

/// Inputs for one making of the Home tab.
pub struct Inputs<'a> {
    pub store: &'a Store,
    pub plays: &'a [PlayRecord],
    pub stars: &'a [StoredEntry],
    pub now: i64,
    pub day: i64,
    pub week: i64,
    /// The saved Home, whose Discover Weekly is kept until its week ends.
    pub previous: Option<&'a Home>,
    /// Make Discover Weekly again even within its week (a refresh).
    pub remake_weekly: bool,
}

pub fn make_home(inputs: &Inputs<'_>) -> Home {
    let store = inputs.store;
    let taste = Taste::from_history(inputs.plays, inputs.stars, inputs.now);
    let mut rng = Rng::new(inputs.day as u64 ^ 0xD15C_0FE5);
    let liked_keys = taste.liked();
    let liked: Vec<Song> = liked_keys
        .iter()
        .take(400)
        .filter_map(|(key, _)| taste.records.get(*key))
        .map(|record| record_song(store, record))
        .collect();
    let (indexed, ..) = store.counts();
    let sample = random_sample(store, &mut rng, 4000, true);

    let mut made_for_you = Vec::new();
    let mut thin = false;
    let groups = clusters(&liked, &mut rng);
    if groups.is_empty() {
        // Nothing heard yet: one mix per system the library has most of.
        thin = true;
        for (index, (system, _)) in store.systems().into_iter().take(MAX_DAILY_MIXES).enumerate() {
            let max = store.max_id().max(1);
            let mut pool = Vec::new();
            for _ in 0..6 {
                let start = 1 + (rng.next() % max as u64) as i64;
                pool.extend(store.songs_of_system(&system, start, 40));
            }
            rng.shuffle(&mut pool);
            let mut blend = Blend::new(1, 3);
            blend.round_robin(vec![pool], DAILY_LENGTH);
            if blend.songs.len() >= 10 {
                made_for_you.push(mix_from(
                    format!("daily-{}", index + 1),
                    "daily",
                    format!("Daily Mix {}", index + 1),
                    match subtitle_for(&blend.songs) {
                        names if names.is_empty() => system.clone(),
                        names => format!("{system} · {names}"),
                    },
                    &blend.songs,
                ));
            }
        }
    }
    for (index, group) in groups.iter().enumerate() {
        let familiar: Vec<Song> = group
            .iter()
            .filter(|song| !taste.recent.contains(&identity(&song.locator)))
            .cloned()
            .collect();
        let sources = new_songs_for(store, group, &sample, &taste, &mut rng);
        let mut fresh = Blend::new(3, 6);
        for song in group {
            fresh.exclude(&song.locator);
        }
        fresh.round_robin(sources, DAILY_LENGTH * 2 / 3);
        // One familiar song, then two new ones.
        let mut songs = Vec::new();
        let mut familiar_iter = familiar.into_iter();
        let mut fresh_iter = fresh.songs.into_iter();
        let familiar_share = DAILY_LENGTH / 3;
        let mut familiar_taken = 0;
        loop {
            let mut progressed = false;
            if familiar_taken < familiar_share
                && let Some(song) = familiar_iter.next()
            {
                songs.push(song);
                familiar_taken += 1;
                progressed = true;
            }
            for _ in 0..2 {
                if let Some(song) = fresh_iter.next() {
                    songs.push(song);
                    progressed = true;
                }
            }
            if !progressed || songs.len() >= DAILY_LENGTH {
                break;
            }
        }
        songs.truncate(DAILY_LENGTH);
        if songs.len() < 8 {
            continue;
        }
        let system = common_system(&songs);
        let names = subtitle_for(&songs);
        let subtitle = match system {
            Some(system) if !names.is_empty() => format!("{system} · {names}"),
            Some(system) => system,
            None => names,
        };
        made_for_you.push(mix_from(
            format!("daily-{}", made_for_you.len() + 1),
            "daily",
            format!("Daily Mix {}", made_for_you.len() + 1),
            subtitle,
            &songs,
        ));
        let _ = index;
    }

    // Discover Weekly: songs never heard, nearest to the listener's styles,
    // by composers both known and new, kept for the week.
    let weekly = match inputs.previous.and_then(Home::weekly) {
        Some(previous)
            if !inputs.remake_weekly
                && inputs.previous.is_some_and(|home| home.week == inputs.week && !home.thin) =>
        {
            Some(previous.clone())
        }
        _ => discover_weekly(store, &taste, &groups, &sample, inputs.week),
    };
    if let Some(weekly) = weekly {
        made_for_you.insert(made_for_you.len().min(1), weekly);
    } else {
        thin = true;
    }

    let mut sections = Vec::new();
    if !made_for_you.is_empty() {
        sections.push(Section { id: "made".into(), title: "Made for you".into(), mixes: made_for_you });
    }
    let albums = unplayed_albums(store, &taste, &liked, &sample, &mut rng);
    if !albums.is_empty() {
        sections.push(Section { id: "albums".into(), title: "Albums you haven't played".into(), mixes: albums });
    }
    let artists = more_by(store, &taste, &mut rng);
    if !artists.is_empty() {
        sections.push(Section { id: "artists".into(), title: "More from composers you like".into(), mixes: artists });
    }
    let mut rediscover = Vec::new();
    if let Some(mix) = forgotten_favorites(store, &taste, inputs.now, &mut rng) {
        rediscover.push(mix);
    }
    if let Some(mix) = year_mix(store, &taste, &sample, &mut rng) {
        rediscover.push(mix);
    }
    if !rediscover.is_empty() {
        sections.push(Section { id: "rediscover".into(), title: "Rediscover".into(), mixes: rediscover });
    }
    if let Some(mix) = new_in_library(store, &taste, inputs.now) {
        sections.push(Section { id: "new".into(), title: "New in your library".into(), mixes: vec![mix] });
    }
    if indexed < 2000 {
        thin = true;
    }
    Home {
        day: inputs.day,
        week: inputs.week,
        made_at: inputs.now,
        sections,
        thin,
        indexed,
    }
}

fn discover_weekly(
    store: &Store,
    taste: &Taste,
    groups: &[Vec<Song>],
    sample: &[Song],
    week: i64,
) -> Option<Mix> {
    let mut rng = Rng::new(week as u64 ^ 0x0EE7_1E55);
    let fresh = |song: &Song| !taste.is_heard(&song.locator) && !taste.disliked(&song.locator);
    let mut sources: Vec<Vec<Song>> = Vec::new();
    // Sounds like each style the listener likes.
    for group in groups {
        let vectors: Vec<&[f32]> = group.iter().filter_map(|song| song.features.as_deref()).collect();
        if let Some(center) = features::centroid(vectors) {
            let pool: Vec<Song> = sample.iter().filter(|song| fresh(song)).cloned().collect();
            sources.push(nearest(&center, &pool, 60));
        }
    }
    // A second hop: composers met in those songs who were never heard,
    // with their other songs.
    let mut branches = Vec::new();
    let mut met = HashSet::new();
    for source in &sources {
        for song in source.iter().take(15) {
            let credit = song.credit();
            if credit.is_empty() || taste.credit_score(credit) > 0.0 || !met.insert(fold(credit)) {
                continue;
            }
            let mut theirs: Vec<Song> =
                store.songs_by_credit(credit, 60).into_iter().filter(|song| fresh(song)).collect();
            rng.shuffle(&mut theirs);
            branches.extend(theirs.into_iter().take(4));
            if met.len() >= 6 {
                break;
            }
        }
    }
    sources.push(branches);
    // Liked composers' songs not heard yet, at most two each.
    let mut known = Vec::new();
    for (_, name, _) in taste.credits.iter().take(20) {
        let mut theirs: Vec<Song> =
            store.songs_by_credit(name, 80).into_iter().filter(|song| fresh(song)).collect();
        rng.shuffle(&mut theirs);
        known.extend(theirs.into_iter().take(2));
    }
    rng.shuffle(&mut known);
    sources.push(known);
    // Anything from the library, for a listener with no history yet.
    let mut anything: Vec<Song> = sample.iter().filter(|song| fresh(song)).cloned().collect();
    if anything.len() < WEEKLY_LENGTH {
        anything.extend(random_sample(store, &mut rng, 400, false).into_iter().filter(|song| fresh(song)));
    }
    sources.push(anything);
    let mut blend = Blend::new(1, 2);
    blend.round_robin(sources, WEEKLY_LENGTH);
    if blend.songs.len() < 10 {
        return None;
    }
    let mut songs = blend.songs;
    rng.shuffle(&mut songs);
    Some(mix_from(
        "weekly".into(),
        "weekly",
        "Discover Weekly".into(),
        with_names("New to you", &songs),
        &songs,
    ))
}

/// Albums with no song heard, by how well they suit the listener: their
/// composers, their systems and how they sound.
fn unplayed_albums(store: &Store, taste: &Taste, liked: &[Song], sample: &[Song], rng: &mut Rng) -> Vec<Mix> {
    let mut candidates: Vec<Song> = Vec::new();
    for (_, name, _) in taste.credits.iter().take(15) {
        candidates.extend(store.songs_by_credit(name, 100));
    }
    let vectors: Vec<&[f32]> = liked.iter().filter_map(|song| song.features.as_deref()).collect();
    let center = features::centroid(vectors);
    if let Some(center) = &center {
        candidates.extend(nearest(center, sample, 300));
    }
    candidates.extend(sample.iter().take(400).cloned());
    let mut scores: HashMap<String, (f64, Song)> = HashMap::new();
    let heard_albums: HashSet<String> = taste
        .records
        .iter()
        .filter(|(key, _)| taste.heard.contains(*key))
        .filter_map(|(_, record)| album_key(&record.album, &record.album_artist))
        .collect();
    for song in candidates {
        let Some(album) = song_album_key(&song) else { continue };
        if heard_albums.contains(&album) || taste.is_heard(&song.locator) {
            continue;
        }
        let mut score = taste.credit_score(song.credit()).min(10.0) * 0.3;
        score += taste.systems.get(&song.system).copied().unwrap_or(0.0).min(10.0) * 0.05;
        if let (Some(center), Some(features)) = (&center, &song.features) {
            score += (1.5 - f64::from(features::distance(center, features))).max(0.0);
        }
        score += (rng.below(1000) as f64) / 4000.0;
        let entry = scores.entry(album).or_insert((0.0, song.clone()));
        entry.0 = entry.0.max(score);
    }
    let mut ranked: Vec<(f64, Song)> = scores.into_values().collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut mixes = Vec::new();
    let mut credits_used = HashSet::new();
    for (_, song) in ranked {
        if mixes.len() >= 8 {
            break;
        }
        let credit = fold(song.credit());
        if !credit.is_empty() && !credits_used.insert(credit) {
            continue;
        }
        let album = song_album_key(&song);
        let mut tracks: Vec<Song> = store
            .songs_by_album(&song.album, 400)
            .into_iter()
            .filter(|other| song_album_key(other) == album)
            .collect();
        if tracks.len() < 3 {
            continue;
        }
        tracks.sort_by(|a, b| a.path.cmp(&b.path).then(a.entry.cmp(&b.entry)).then(a.id.cmp(&b.id)));
        let mut details = Vec::new();
        if !song.credit().is_empty() {
            details.push(song.credit().to_owned());
        }
        if let Some(year) = song.year {
            details.push(year.to_string());
        }
        if !song.system.is_empty() {
            details.push(song.system.clone());
        }
        mixes.push(mix_from(
            format!("album-{}", mixes.len() + 1),
            "album",
            song.album.clone(),
            details.join(" · "),
            &tracks,
        ));
    }
    mixes
}

/// Songs by the composers the listener likes most that they have not heard.
fn more_by(store: &Store, taste: &Taste, rng: &mut Rng) -> Vec<Mix> {
    let mut mixes = Vec::new();
    for (_, name, _) in &taste.credits {
        if mixes.len() >= 4 {
            break;
        }
        let mut songs: Vec<Song> = store
            .songs_by_credit(name, 400)
            .into_iter()
            .filter(|song| !taste.is_heard(&song.locator) && !taste.disliked(&song.locator))
            .collect();
        rng.shuffle(&mut songs);
        let mut blend = Blend::new(4, usize::MAX);
        blend.round_robin(vec![songs], 30);
        if blend.songs.len() < 8 {
            continue;
        }
        mixes.push(mix_from(
            format!("artist-{}", mixes.len() + 1),
            "artist",
            format!("More by {name}"),
            String::new(),
            &blend.songs,
        ));
    }
    mixes
}

/// Songs once played often, not heard for two months.
fn forgotten_favorites(store: &Store, taste: &Taste, now: i64, rng: &mut Rng) -> Option<Mix> {
    let mut forgotten: Vec<(&String, f64)> = taste
        .songs
        .iter()
        .filter(|(key, _)| {
            taste.last_heard.get(*key).is_some_and(|last| now - last > 60 * DAY_MS)
        })
        .map(|(key, score)| (key, *score))
        .collect();
    // Scores faded with age, so weigh them back up by how long ago.
    forgotten.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let mut songs: Vec<Song> = forgotten
        .iter()
        .take(120)
        .filter_map(|(key, _)| taste.records.get(*key))
        .map(|record| record_song(store, record))
        .collect();
    rng.shuffle(&mut songs);
    let mut blend = Blend::new(2, 4);
    blend.round_robin(vec![songs], 30);
    (blend.songs.len() >= 8).then(|| {
        mix_from(
            "forgotten".into(),
            "rediscover",
            "Forgotten favorites".into(),
            with_names("Not heard in a while", &blend.songs),
            &blend.songs,
        )
    })
}

/// The year the listener likes most, with songs from it they have not heard.
fn year_mix(store: &Store, taste: &Taste, sample: &[Song], rng: &mut Rng) -> Option<Mix> {
    let (year, _) = taste
        .years
        .iter()
        .filter(|(_, score)| **score > 1.0)
        .max_by(|a, b| a.1.total_cmp(b.1))?;
    let mut pool: Vec<Song> = sample
        .iter()
        .filter(|song| song.year == Some(*year) && !taste.is_heard(&song.locator))
        .cloned()
        .collect();
    for (_, name, _) in taste.credits.iter().take(10) {
        pool.extend(
            store
                .songs_by_credit(name, 200)
                .into_iter()
                .filter(|song| song.year == Some(*year) && !taste.is_heard(&song.locator)),
        );
    }
    rng.shuffle(&mut pool);
    let mut blend = Blend::new(2, 3);
    blend.round_robin(vec![pool], 30);
    (blend.songs.len() >= 8).then(|| {
        mix_from(
            "year".into(),
            "year",
            format!("Back to {year}"),
            subtitle_for(&blend.songs),
            &blend.songs,
        )
    })
}

/// Files that appeared in the last month and have not been heard.
fn new_in_library(store: &Store, taste: &Taste, now: i64) -> Option<Mix> {
    let songs: Vec<Song> = store
        .newest(600)
        .into_iter()
        .filter(|song| song.added_at > now - 30 * DAY_MS && !taste.is_heard(&song.locator))
        .collect();
    let mut blend = Blend::new(3, 6);
    blend.round_robin(vec![songs], 40);
    (blend.songs.len() >= 8).then(|| {
        mix_from(
            "new".into(),
            "new",
            "Fresh in your library".into(),
            subtitle_for(&blend.songs),
            &blend.songs,
        )
    })
}

/// The local day number of `now` (days since the epoch, counted from local
/// midnight) and the day its week began (Monday).
pub fn local_day_and_week(now: i64) -> (i64, i64) {
    let local = now + local_offset_ms(now);
    let day = local.div_euclid(DAY_MS);
    // 1970-01-01 was a Thursday: Monday is three days earlier.
    let weekday = (day + 3).rem_euclid(7);
    (day, day - weekday)
}

/// How far the local clock is ahead of UTC, so days start at local
/// midnight. The docker image ships time zone data for this.
#[cfg(unix)]
fn local_offset_ms(now: i64) -> i64 {
    let seconds = (now / 1000) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&seconds, &mut tm) }.is_null() {
        return 0;
    }
    tm.tm_gmtoff as i64 * 1000
}

#[cfg(not(unix))]
fn local_offset_ms(_now: i64) -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(locator: &str, composer: &str, album: &str, at: i64, listened: i64, finished: bool) -> PlayRecord {
        PlayRecord {
            locator: locator.into(),
            kind: "local".into(),
            path: locator.into(),
            played_at: at,
            listened_ms: listened,
            length_ms: 120_000,
            finished,
            title: locator.into(),
            composer: composer.into(),
            album: album.into(),
            codec: "NSF / Game_Music_Emu".into(),
            ..PlayRecord::default()
        }
    }

    #[test]
    fn plays_heard_count_for_and_skips_against() {
        let now = 100 * DAY_MS;
        let plays = vec![
            play("/a.nsf#0", "Kondo", "A", now - DAY_MS, 120_000, true),
            play("/a.nsf#0", "Kondo", "A", now - DAY_MS, 120_000, true),
            play("/b.nsf#0", "Tanaka", "B", now - DAY_MS, 3_000, false),
            play("/b.nsf#0", "Tanaka", "B", now - DAY_MS, 3_000, false),
        ];
        let taste = Taste::from_history(&plays, &[], now);
        assert!(taste.songs["/a.nsf"] > 1.5);
        assert!(taste.disliked("/b.nsf#0"));
        assert!(taste.is_heard("/a.nsf"));
        assert!(!taste.is_heard("/b.nsf#0"));
        assert_eq!(taste.credits[0].1, "Kondo");
        assert!(taste.systems.contains_key("NSF"));
    }

    #[test]
    fn a_blend_caps_albums_and_credits() {
        let song = |n: usize, album: &str, credit: &str| Song {
            locator: format!("/{n}"),
            album: album.into(),
            composer: credit.into(),
            ..Song::default()
        };
        let one: Vec<Song> = (0..10).map(|n| song(n, "Same", "X")).collect();
        let two: Vec<Song> = (10..20).map(|n| song(n, &format!("Album {n}"), "Y")).collect();
        let mut blend = Blend::new(2, 3);
        blend.round_robin(vec![one, two], 30);
        let same = blend.songs.iter().filter(|s| s.album == "Same").count();
        let y = blend.songs.iter().filter(|s| s.composer == "Y").count();
        assert_eq!(same, 2);
        assert_eq!(y, 3);
    }

    #[test]
    fn weeks_begin_on_monday() {
        // Day 20773 (2026-11-16) was a Monday; noon keeps the test clear of
        // any time zone's midnight.
        let monday_noon = 20_773 * DAY_MS + DAY_MS / 2;
        let (day, week) = local_day_and_week(monday_noon + DAY_MS * 3);
        assert_eq!(day - week, 3);
        assert_eq!((week + 3).rem_euclid(7), 0);
    }

    #[test]
    fn a_new_index_makes_mixes_per_system() {
        let mut store = Store::open_in_memory().unwrap();
        for file in 0..60 {
            let songs: Vec<Song> = (0..4)
                .map(|sub| Song {
                    locator: format!("/f{file}.nsf#{sub}"),
                    file_key: format!("/f{file}.nsf"),
                    kind: "local".into(),
                    path: format!("/f{file}.nsf"),
                    fragment: Some(sub.to_string()),
                    title: format!("Song {sub}"),
                    album: format!("Game {file}"),
                    composer: format!("Composer {}", file % 7),
                    system: if file % 2 == 0 { "NSF".into() } else { "SPC".into() },
                    ..Song::default()
                })
                .collect();
            store.put_file(&format!("/f{file}.nsf"), "1", &songs).unwrap();
        }
        let (day, week) = local_day_and_week(1_800_000_000_000);
        let home = make_home(&Inputs {
            store: &store,
            plays: &[],
            stars: &[],
            now: 1_800_000_000_000,
            day,
            week,
            previous: None,
            remake_weekly: false,
        });
        assert!(home.thin);
        let made = &home.sections[0];
        assert!(made.mixes.iter().any(|mix| mix.id == "weekly"));
        assert!(made.mixes.iter().filter(|mix| mix.kind == "daily").count() >= 2);
        for mix in &made.mixes {
            let mut albums: HashMap<&str, usize> = HashMap::new();
            for song in &mix.songs {
                *albums.entry(song.album.as_str()).or_insert(0) += 1;
            }
            assert!(albums.values().all(|count| *count <= 1), "{}", mix.title);
        }
    }
}
