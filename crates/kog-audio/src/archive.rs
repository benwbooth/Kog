//! Safe temporary extraction for Cog-compatible audio archives.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use compress_tools::{ArchiveContents, ArchiveIteratorBuilder};
use tempfile::TempDir;

const MAX_ENTRIES: usize = 16_384;
const MAX_ENTRY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_ROM_ENTRIES: usize = 256;
const MAX_ROM_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ROM_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const FILE_TYPE_MASK: u32 = 0o170_000;
const FILE_TYPE_DIRECTORY: u32 = 0o040_000;
const FILE_TYPE_REGULAR: u32 = 0o100_000;
const GENERAL_ARCHIVE_EXTENSIONS: &[&str] = &["zip", "rar", "7z", "rsn", "vgm7z", "gz"];
pub const COG_OPENMPT_ARCHIVE_EXTENSIONS: &[&str] = &["mdz", "mdr", "s3z", "xmz", "itz", "mptmz"];
const ARCHIVE_NAME_CACHE_BYTES: usize = 256 * 1024 * 1024;

struct CachedNames {
    size: u64,
    modified: SystemTime,
    names: Arc<Vec<String>>,
    bytes: usize,
    sequence: u64,
}

#[derive(Default)]
struct ArchiveNameCache {
    entries: HashMap<PathBuf, CachedNames>,
    order: VecDeque<(PathBuf, u64)>,
    bytes: usize,
    sequence: u64,
}

fn archive_name_cache() -> &'static Mutex<ArchiveNameCache> {
    static CACHE: OnceLock<Mutex<ArchiveNameCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ArchiveNameCache::default()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct ExtractedArchive {
    pub entries: Vec<ArchiveEntry>,
    pub warnings: Vec<String>,
    temporary_directory: TempDir,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TreeLocation {
    pub archive: PathBuf,
    pub entry: String,
    pub directory: bool,
}

pub fn is_tree_location(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|path| path.starts_with("kog-archive:"))
}

pub fn tree_location(path: &Path) -> Result<Option<TreeLocation>, String> {
    if !is_tree_location(path) {
        return Ok(None);
    }
    let url = url::Url::parse(path.to_str().unwrap()).map_err(|e| e.to_string())?;
    let mut archive = None;
    let mut entry = None;
    let mut directory = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "archive" if archive.is_none() => archive = Some(PathBuf::from(value.as_ref())),
            "entry" if entry.is_none() => entry = Some(value.into_owned()),
            "directory" if directory.is_none() && matches!(value.as_ref(), "0" | "1") => {
                directory = Some(value == "1")
            }
            _ => return Err("Invalid archive tree location".into()),
        }
    }
    let archive = archive
        .filter(|p| p.is_absolute() && is_path(p))
        .ok_or("Invalid archive tree source")?;
    let entry = entry.ok_or("Missing archive entry")?;
    let entry = portable_name(&safe_relative_path(&entry)?);
    Ok(Some(TreeLocation {
        archive,
        entry,
        directory: directory.ok_or("Missing archive entry type")?,
    }))
}

impl ExtractedArchive {
    pub fn root(&self) -> &Path {
        self.temporary_directory.path()
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        if !is_path(path) {
            return Err(format!(
                "{} is not a supported audio archive",
                path.display()
            ));
        }

        Self::open_with_limits(path, MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_TOTAL_BYTES)
    }

    pub fn open_rom(path: &Path) -> Result<Self, String> {
        Self::open_with_limits(
            path,
            MAX_ROM_ENTRIES,
            MAX_ROM_ENTRY_BYTES,
            MAX_ROM_TOTAL_BYTES,
        )
    }

    pub fn open_skin(path: &Path) -> Result<Self, String> {
        Self::open_with_limits(path, 512, 8 * 1024 * 1024, 32 * 1024 * 1024)
    }

    fn open_with_limits(
        path: &Path,
        max_entries: usize,
        max_entry_bytes: u64,
        max_total_bytes: u64,
    ) -> Result<Self, String> {
        let source = File::open(path)
            .map_err(|error| format!("opening archive {}: {error}", path.display()))?;
        let temporary_directory = tempfile::Builder::new()
            .prefix("kog-archive-")
            .tempdir()
            .map_err(|error| format!("creating archive workspace: {error}"))?;
        let raw_stream = extension(path).is_some_and(|value| {
            ["gz", "bz2", "xz", "lzma"]
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(value))
        });
        // Some archives store a directory as a bare, regular-looking member.
        // The names of its descendants are the reliable way to identify it.
        let member_directories = if raw_stream {
            HashSet::new()
        } else {
            member_directory_names(&list_archive_names(path)?)
        };
        let iterator = ArchiveIteratorBuilder::new(source)
            .decoder(decode_archive_name)
            .mtree_format(false)
            .raw_format(raw_stream)
            .build()
            .map_err(|error| format!("reading archive {}: {error}", path.display()))?;

        let mut entries = Vec::new();
        let mut warnings = Vec::new();
        let mut seen_paths = HashSet::new();
        let mut current = CurrentEntry::None;
        let mut total_bytes = 0_u64;
        let mut entry_count = 0_usize;

        for content in iterator {
            match content {
                ArchiveContents::StartOfEntry(name, stat) => {
                    if !matches!(current, CurrentEntry::None) {
                        return Err(format!(
                            "archive {} started a new entry before ending the previous one",
                            path.display()
                        ));
                    }
                    entry_count += 1;
                    if entry_count > max_entries {
                        return Err(format!(
                            "archive {} exceeds Kog's {max_entries}-entry safety limit",
                            path.display(),
                        ));
                    }

                    let name = raw_entry_name(path, &name, raw_stream);
                    let relative = match safe_relative_path(&name) {
                        Ok(relative) => relative,
                        Err(error) => {
                            warnings.push(format!("Skipped archive entry {name:?}: {error}"));
                            current = CurrentEntry::Discard { written: 0 };
                            continue;
                        }
                    };
                    if kog_core::media_path::is_metadata(&relative) {
                        current = CurrentEntry::Discard { written: 0 };
                        continue;
                    }
                    if !seen_paths.insert(relative.clone()) {
                        warnings.push(format!(
                            "Skipped duplicate archive entry {}",
                            portable_name(&relative)
                        ));
                        current = CurrentEntry::Discard { written: 0 };
                        continue;
                    }

                    let mode = u32::from(stat.st_mode) & FILE_TYPE_MASK;
                    let named_directory = name.ends_with('/') || name.ends_with('\\');
                    let is_directory = mode == FILE_TYPE_DIRECTORY
                        || named_directory
                        || member_directories.contains(&portable_name(&relative));
                    let is_regular = mode == 0 || mode == FILE_TYPE_REGULAR;
                    let target = temporary_directory.path().join(&relative);
                    if is_directory {
                        std::fs::create_dir_all(&target).map_err(|error| {
                            format!("creating archive directory {}: {error}", target.display())
                        })?;
                        current = CurrentEntry::Discard { written: 0 };
                        continue;
                    }
                    if !is_regular {
                        warnings.push(format!(
                            "Skipped non-regular archive entry {}",
                            portable_name(&relative)
                        ));
                        current = CurrentEntry::Discard { written: 0 };
                        continue;
                    }
                    if stat.st_size > 0 && stat.st_size as u64 > max_entry_bytes {
                        return Err(format!(
                            "archive entry {} exceeds Kog's {} MiB per-file safety limit",
                            portable_name(&relative),
                            max_entry_bytes / 1024 / 1024,
                        ));
                    }
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent).map_err(|error| {
                            format!("creating archive directory {}: {error}", parent.display())
                        })?;
                    }
                    let file = OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(&target)
                        .map_err(|error| {
                            format!("creating archive entry {}: {error}", target.display())
                        })?;
                    entries.push(ArchiveEntry {
                        name: portable_name(&relative),
                        path: target,
                    });
                    current = CurrentEntry::File { file, written: 0 };
                }
                ArchiveContents::DataChunk(bytes) => {
                    let chunk_size = u64::try_from(bytes.len())
                        .map_err(|_| "archive data chunk is too large".to_owned())?;
                    total_bytes = total_bytes
                        .checked_add(chunk_size)
                        .ok_or_else(|| "archive expanded size overflowed".to_owned())?;
                    if total_bytes > max_total_bytes {
                        return Err(format!(
                            "archive {} exceeds Kog's {} MiB expanded-size safety limit",
                            path.display(),
                            max_total_bytes / 1024 / 1024,
                        ));
                    }
                    match &mut current {
                        CurrentEntry::File { file, written } => {
                            *written = written
                                .checked_add(chunk_size)
                                .ok_or_else(|| "archive entry size overflowed".to_owned())?;
                            if *written > max_entry_bytes {
                                return Err(format!(
                                    "an entry in {} exceeds Kog's {} MiB per-file safety limit",
                                    path.display(),
                                    max_entry_bytes / 1024 / 1024,
                                ));
                            }
                            file.write_all(&bytes).map_err(|error| {
                                format!("writing extracted data from {}: {error}", path.display())
                            })?;
                        }
                        CurrentEntry::Discard { written } => *written += chunk_size,
                        CurrentEntry::None => {
                            return Err(format!(
                                "archive {} produced data outside an entry",
                                path.display()
                            ));
                        }
                    }
                }
                ArchiveContents::EndOfEntry => {
                    if let CurrentEntry::File { file, .. } = &mut current {
                        file.flush().map_err(|error| {
                            format!("flushing extracted data from {}: {error}", path.display())
                        })?;
                    }
                    current = CurrentEntry::None;
                }
                ArchiveContents::Err(error) => {
                    return Err(format!("extracting archive {}: {error}", path.display()));
                }
            }
        }
        if !matches!(current, CurrentEntry::None) {
            return Err(format!("archive {} ended inside an entry", path.display()));
        }

        Ok(Self {
            entries,
            warnings,
            temporary_directory,
        })
    }

    pub fn into_parts(self) -> (TempDir, Vec<ArchiveEntry>, Vec<String>) {
        (self.temporary_directory, self.entries, self.warnings)
    }
}

enum CurrentEntry {
    None,
    File { file: File, written: u64 },
    Discard { written: u64 },
}

pub fn is_path(path: &Path) -> bool {
    extension(path).is_some_and(|extension| {
        GENERAL_ARCHIVE_EXTENSIONS
            .iter()
            .chain(COG_OPENMPT_ARCHIVE_EXTENSIONS)
            .any(|candidate| candidate.eq_ignore_ascii_case(extension))
    })
}

pub fn supported_extensions() -> Vec<String> {
    GENERAL_ARCHIVE_EXTENSIONS
        .iter()
        .chain(COG_OPENMPT_ARCHIVE_EXTENSIONS)
        .map(|extension| (*extension).to_owned())
        .collect()
}

pub const MAX_NESTED_DEPTH: usize = 4;
pub const MAX_NESTED_MEMBER_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_NESTED_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const NESTED_CACHE_SUBDIRECTORY: &str = "nested-archives";

pub fn nested_cache_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("KOG_NESTED_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    directories::ProjectDirs::from("org", "Kog", "Kog")
        .map(|directories| directories.cache_dir().join(NESTED_CACHE_SUBDIRECTORY))
        .unwrap_or_else(|| std::env::temp_dir().join("kog-nested-archives"))
}

/// Tree URL addressing one archive member for browsing and expansion.
pub fn member_url(archive: &Path, entry: &str, directory: bool) -> PathBuf {
    let mut url = url::Url::parse("kog-archive:").expect("kog-archive scheme");
    url.query_pairs_mut()
        .append_pair("archive", &archive.to_string_lossy())
        .append_pair("entry", entry)
        .append_pair("directory", if directory { "1" } else { "0" });
    PathBuf::from(url.as_str())
}

/// Index-only member names of an archive file, without extracting anything.
/// Uses the same name decoder as extraction so listings match extracted
/// entry names byte-for-byte.
pub fn list_archive_names(path: &Path) -> Result<Vec<String>, String> {
    list_archive_names_shared(path).map(|names| names.as_ref().clone())
}

/// Share a bounded name index across searches and archive browsing. The
/// archive's size and modification time invalidate stale entries.
pub fn list_archive_names_shared(path: &Path) -> Result<Arc<Vec<String>>, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("reading archive {}: {error}", path.display()))?;
    let modified = metadata.modified().ok();
    if let Some(modified) = modified {
        let mut cache = archive_name_cache().lock().unwrap();
        if let Some(hit) = cache.entries.get(path) {
            if hit.size == metadata.len() && hit.modified == modified {
                return Ok(hit.names.clone());
            }
        }
        if let Some(stale) = cache.entries.remove(path) {
            cache.bytes = cache.bytes.saturating_sub(stale.bytes);
            cache.order.retain(|(cached_path, _)| cached_path != path);
        }
    }

    // Archive I/O must run outside the cache lock: multiple search workers
    // can list unrelated archives in parallel.
    let names = Arc::new(list_archive_names_uncached(path)?);
    let bytes = path.as_os_str().len()
        + names.iter().map(|name| name.len() + size_of::<String>()).sum::<usize>();
    if bytes > ARCHIVE_NAME_CACHE_BYTES {
        return Ok(names);
    }
    if let Some(modified) = modified
        && std::fs::metadata(path)
            .ok()
            .is_some_and(|current| current.len() == metadata.len()
                && current.modified().ok() == Some(modified))
    {
        let mut cache = archive_name_cache().lock().unwrap();
        if let Some(hit) = cache.entries.get(path)
            && hit.size == metadata.len()
            && hit.modified == modified
        {
            return Ok(hit.names.clone());
        }
        if let Some(old) = cache.entries.remove(path) {
            cache.bytes = cache.bytes.saturating_sub(old.bytes);
            cache.order.retain(|(cached_path, _)| cached_path != path);
        }
        while cache.bytes.saturating_add(bytes) > ARCHIVE_NAME_CACHE_BYTES {
            let Some((old_path, sequence)) = cache.order.pop_front() else {
                break;
            };
            if cache.entries.get(&old_path).is_some_and(|entry| entry.sequence == sequence)
                && let Some(old) = cache.entries.remove(&old_path)
            {
                cache.bytes = cache.bytes.saturating_sub(old.bytes);
            }
        }
        cache.sequence = cache.sequence.wrapping_add(1);
        let sequence = cache.sequence;
        cache.bytes += bytes;
        cache.entries.insert(path.to_path_buf(), CachedNames {
            size: metadata.len(),
            modified,
            names: names.clone(),
            bytes,
            sequence,
        });
        cache.order.push_back((path.to_path_buf(), sequence));
    }
    Ok(names)
}

fn list_archive_names_uncached(path: &Path) -> Result<Vec<String>, String> {
    // ZIP keeps its complete name index at the end of the file. Reading that
    // index avoids streaming every compressed member through libarchive for
    // each library search. Keep libarchive as the compatibility path for
    // malformed or unusual ZIP variants and every other archive format.
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
        && let Ok(file) = File::open(path)
        && let Ok(names) = list_zip_central_directory_names(file)
    {
        return Ok(names);
    }
    let file =
        File::open(path).map_err(|error| format!("opening archive {}: {error}", path.display()))?;
    compress_tools::list_archive_files_with_encoding(file, decode_archive_name)
        .map_err(|error| format!("listing archive {}: {error}", path.display()))
}

fn list_zip_central_directory_names(file: File) -> Result<Vec<String>, zip::result::ZipError> {
    let mut archive = zip::ZipArchive::new(file)?;
    let mut names = Vec::new();
    for index in 0..archive.len() {
        // The central directory exposes ASCII names without a seek to each
        // member's local header. Decode raw bytes only for non-ASCII names so
        // legacy filename encodings keep Kog's existing heuristic behavior.
        if let Some(name) = archive.name_for_index(index).filter(|name| name.is_ascii()) {
            names.push(name.to_owned());
        } else {
            let member = archive.by_index_raw(index)?;
            names.push(kog_core::text_encoding::decode(member.name_raw()));
        }
    }
    Ok(names)
}

/// Directory names inferred from member paths, including explicit records
/// whose stored name has no trailing slash. Browsing and extraction use this
/// same classification so they never disagree about a playable member.
pub fn member_directory_names(members: &[String]) -> HashSet<String> {
    let mut directories = HashSet::new();
    for name in members {
        let Ok(relative) = safe_relative_path(name) else {
            continue;
        };
        let mut parent = relative.parent();
        while let Some(path) = parent.filter(|path| !path.as_os_str().is_empty()) {
            directories.insert(portable_name(path));
            parent = path.parent();
        }
        if name.ends_with('/') || name.ends_with('\\') {
            directories.insert(portable_name(&relative));
        }
    }
    directories
}

/// Longest leading run of `entry` components naming an archive file member
/// of the listed container. Returns (archive link, remainder), with the link
/// in its original stored spelling for extraction. An explicit
/// trailing-slash directory entry always wins ties: real folders are never
/// descended into.
fn longest_archive_prefix(members: &[String], entry: &str) -> Option<(String, String)> {
    let normalized = entry.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();
    for length in (1..=parts.len()).rev() {
        let candidate = parts[..length].join("/");
        if candidate.is_empty() {
            continue;
        }
        if members
            .iter()
            .any(|member| member == &format!("{candidate}/"))
        {
            continue;
        }
        if !is_path(Path::new(&candidate)) {
            continue;
        }
        if let Some(original) = members.iter().find(|member| {
            member.trim_end_matches('/') == candidate
                || member.replace('\\', "/").trim_end_matches('/') == candidate
        }) {
            let remainder = parts[length..].join("/");
            // Extract with the stored spelling; match with the normalized one.
            let link = original.trim_end_matches('/').to_owned();
            return Some((link, remainder));
        }
    }
    None
}

struct CappedWriter<W: Write> {
    inner: W,
    remaining: u64,
}

impl<W: Write> Write for CappedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::QuotaExceeded,
                "archive member exceeds the nested-archive size limit",
            ));
        }
        let allowed = usize::try_from(self.remaining.min(bytes.len() as u64)).unwrap_or(usize::MAX);
        let written = self.inner.write(&bytes[..allowed])?;
        self.remaining = self.remaining.saturating_sub(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn container_fingerprint(container: &Path) -> Result<(u64, i128), String> {
    let metadata = container.metadata().map_err(|error| {
        format!("reading archive {}: {error}", container.display())
    })?;
    let modified = metadata
        .modified()
        .map_err(|error| format!("reading archive {}: {error}", container.display()))?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|age| age.as_millis() as i128)
        .unwrap_or_default();
    Ok((metadata.len(), modified))
}

fn nested_cache_name(container: &Path, link: &str, length: u64, modified: i128) -> String {
    let extension = Path::new(link)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin");
    let stem: String = Path::new(link)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("archive")
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let fingerprint = format!("{}|{length}|{modified}|{link}", container.display());
    let hash = crate::cover_art::fnv1a64(fingerprint.as_bytes());
    format!("{stem}-{length}-{modified}-{hash:016x}.{extension}")
}

/// Extract one archive member to a stable cache file keyed by the outer
/// container's identity, so playlists and the tree keep working across
/// sessions. Reuses the cached copy while the outer file is unchanged.
/// Partial downloads are written aside and renamed only on success.
pub fn materialize_archive_member(
    container: &Path,
    link: &str,
    cache_dir: &Path,
) -> Result<PathBuf, String> {
    let (length, modified) = container_fingerprint(container)?;
    let path = cache_dir.join(nested_cache_name(container, link, length, modified));
    if path.is_file() {
        return Ok(path);
    }
    std::fs::create_dir_all(cache_dir).map_err(|error| {
        format!(
            "creating archive cache {}: {error}",
            cache_dir.display()
        )
    })?;
    let source = File::open(container).map_err(|error| {
        format!("opening archive {}: {error}", container.display())
    })?;
    let partial = path.with_extension("part");
    let target = File::create(&partial).map_err(|error| {
        format!("writing archive cache {}: {error}", partial.display())
    })?;
    let capped = CappedWriter {
        inner: target,
        remaining: MAX_NESTED_MEMBER_BYTES,
    };
    if let Err(error) = compress_tools::uncompress_archive_file_with_encoding(
        source,
        capped,
        link,
        decode_archive_name,
    ) {
        let _ = std::fs::remove_file(&partial);
        return Err(format!(
            "reading {link} from {}: {error}",
            container.display()
        ));
    }
    std::fs::rename(&partial, &path)
        .map_err(|error| format!("writing archive cache {}: {error}", path.display()))?;
    crate::cover_art::evict_cache(cache_dir, MAX_NESTED_CACHE_BYTES);
    Ok(path)
}

/// Resolved nested location: the deepest container file holding the leaf,
/// plus the stable outer identity for playlist origins. `prefix` is the
/// outer-relative path of `container` ("" when it is the outer archive
/// itself); `links` counts consumed archive links for depth accounting.
#[derive(Clone, Debug)]
pub struct ResolvedArchive {
    pub outer: PathBuf,
    pub container: PathBuf,
    pub leaf: String,
    pub prefix: String,
    pub links: u32,
}

/// Resolve (outer archive, full member path) to the deepest container file
/// and leaf remainder, materializing nested archives to the stable cache.
/// Refuses to pass MAX_NESTED_DEPTH archive links so hostile self-nesting
/// terminates loudly.
pub fn resolve_archive_chain(
    outer: &Path,
    entry: &str,
    cache_dir: &Path,
) -> Result<ResolvedArchive, String> {
    let canonical = outer
        .canonicalize()
        .map_err(|error| format!("resolving archive {}: {error}", outer.display()))?;
    let mut current = canonical.clone();
    let mut rest = entry.replace('\\', "/");
    let mut consumed: Vec<String> = Vec::new();
    for _ in 0..MAX_NESTED_DEPTH {
        let members = list_archive_names(&current)?;
        let Some((link, remainder)) = longest_archive_prefix(&members, &rest) else {
            return Ok(ResolvedArchive {
                outer: canonical,
                container: current,
                leaf: rest,
                prefix: consumed.join("/"),
                links: consumed.len() as u32,
            });
        };
        current = materialize_archive_member(&current, &link, cache_dir)?;
        consumed.push(link);
        rest = remainder;
    }
    let members = list_archive_names(&current)?;
    if longest_archive_prefix(&members, &rest).is_some() {
        return Err("archive nesting exceeds Kog's 4-level safety limit".to_owned());
    }
    Ok(ResolvedArchive {
        outer: canonical,
        container: current,
        leaf: rest,
        prefix: consumed.join("/"),
        links: consumed.len() as u32,
    })
}

fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|value| value.to_str())
}

fn raw_entry_name(archive: &Path, name: &str, raw_stream: bool) -> String {
    if !raw_stream || name != "data" {
        return name.to_owned();
    }
    archive
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("data")
        .to_owned()
}

fn safe_relative_path(name: &str) -> Result<PathBuf, String> {
    let normalized = name.replace('\\', "/");
    let bytes = normalized.as_bytes();
    if normalized.starts_with('/')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
    {
        return Err("absolute paths are not allowed".to_owned());
    }

    let mut path = PathBuf::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(value) => path.push(value),
            Component::CurDir => {}
            Component::ParentDir => return Err("parent traversal is not allowed".to_owned()),
            Component::Prefix(_) | Component::RootDir => {
                return Err("absolute paths are not allowed".to_owned());
            }
        }
    }
    if path.as_os_str().is_empty() {
        return Err("empty paths are not allowed".to_owned());
    }
    Ok(path)
}

pub fn portable_name(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn decode_archive_name(bytes: &[u8]) -> compress_tools::Result<String> {
    Ok(kog_core::text_encoding::decode(bytes))
}

#[cfg(any(test, feature = "test-util"))]
pub mod tests {
    use super::*;
    use crate::decoder::{ArchiveOrigin, DecoderRegistry, DecoderSettings, PlaybackSource};
    use crate::gsf::{test_gba_rom, test_gsf_bytes};
    use crate::ncsf::{test_ncsf_bytes, test_sdat_bytes};
    use crate::playlist::{Playlist, PlaylistEntry, PlaylistLocation};
    use crate::psf::{
        test_psf_bytes, test_psf_executable, test_psf2_bytes, test_psf2_irx, test_snsf_bytes,
        test_snsf_rom, test_twosf_bytes, test_twosf_rom,
    };
    use crate::qsf::{test_qsf_bytes, test_qsf_program};
    use crate::sdsf::{test_sdsf_bytes, test_ssf_program};
    use crate::settings::MidiEngine;
    use crate::syntrax::test_jxs_bytes;
    use crate::usf::{test_usf_bytes, test_usf_reserved};

    // Generated with libarchive 3.8.9 from an empty regular file. It exercises
    // real 7Z parsing without requiring an archive-writing tool during tests.
    const EMPTY_7Z: &[u8] = &[
        0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c, 0x00, 0x03, 0xe7, 0x33, 0x3e, 0x74, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x4e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xed, 0x80,
        0x6c, 0xe8, 0x01, 0x05, 0x01, 0x0e, 0x01, 0x80, 0x0f, 0x01, 0x80, 0x11, 0x15, 0x00, 0x65,
        0x00, 0x6d, 0x00, 0x70, 0x00, 0x74, 0x00, 0x79, 0x00, 0x2e, 0x00, 0x74, 0x00, 0x78, 0x00,
        0x74, 0x00, 0x00, 0x00, 0x14, 0x0a, 0x01, 0x00, 0x1d, 0xbf, 0x4b, 0x1b, 0x1d, 0x39, 0xdd,
        0x01, 0x12, 0x0a, 0x01, 0x00, 0x1d, 0xbf, 0x4b, 0x1b, 0x1d, 0x39, 0xdd, 0x01, 0x13, 0x0a,
        0x01, 0x00, 0x8b, 0x29, 0x3a, 0xe8, 0x25, 0x39, 0xdd, 0x01, 0x15, 0x06, 0x01, 0x00, 0x20,
        0x80, 0xb4, 0x81, 0x00, 0x00,
    ];

    // libarchive's BSD-licensed RAR5 stored fixture, reduced to its decoded
    // 109-byte archive from test_read_format_rar5_stored.rar.uu. Attribution
    // and its two-clause license are recorded in THIRD_PARTY_NOTICES.md.
    const STORED_RAR5: &[u8] = &[
        0x52, 0x61, 0x72, 0x21, 0x1a, 0x07, 0x01, 0x00, 0x33, 0x92, 0xb5, 0xe5, 0x0a, 0x01, 0x05,
        0x06, 0x00, 0x05, 0x01, 0x01, 0x80, 0x80, 0x00, 0x38, 0x30, 0x06, 0x63, 0x2c, 0x02, 0x03,
        0x0b, 0x9d, 0x00, 0x04, 0x9d, 0x00, 0xa4, 0x83, 0x02, 0xb4, 0x43, 0xa0, 0x95, 0x80, 0x00,
        0x01, 0x0e, 0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x77, 0x6f, 0x72, 0x6c, 0x64, 0x2e, 0x74, 0x78,
        0x74, 0x0a, 0x03, 0x13, 0x7e, 0x0e, 0xab, 0x5b, 0x56, 0xe9, 0x0e, 0x1a, 0x68, 0x65, 0x6c,
        0x6c, 0x6f, 0x20, 0x6c, 0x69, 0x62, 0x61, 0x72, 0x63, 0x68, 0x69, 0x76, 0x65, 0x20, 0x74,
        0x65, 0x73, 0x74, 0x20, 0x73, 0x75, 0x69, 0x74, 0x65, 0x21, 0x0a, 0x1d, 0x77, 0x56, 0x51,
        0x03, 0x05, 0x04, 0x00,
    ];

    pub fn wav_bytes(seed: i16) -> Vec<u8> {
        const SAMPLE_RATE: u32 = 8_000;
        const FRAMES: u32 = 80;
        let data_size = FRAMES * 2;
        let mut bytes = Vec::with_capacity((44 + data_size) as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        for frame in 0..FRAMES {
            bytes.extend_from_slice(&(seed + frame as i16 * 100).to_le_bytes());
        }
        bytes
    }

    fn format_two_midi_bytes() -> Vec<u8> {
        let mut midi = vec![b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 2, 0, 2, 1, 0xe0];
        for (name, note, duration) in [
            ("Archive Opening", 60_u8, [0x83, 0x60]),
            ("Archive Finale", 67_u8, [0x87, 0x40]),
        ] {
            let mut track = vec![0, 0xff, 0x03, name.len() as u8];
            track.extend_from_slice(name.as_bytes());
            track.extend_from_slice(&[0, 0xc0, 0, 0, 0x90, note, 100]);
            track.extend_from_slice(&duration);
            track.extend_from_slice(&[0x80, note, 64, 0, 0xff, 0x2f, 0]);
            midi.extend_from_slice(b"MTrk");
            midi.extend_from_slice(&(track.len() as u32).to_be_bytes());
            midi.extend_from_slice(&track);
        }
        midi
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                let mask = 0_u32.wrapping_sub(crc & 1);
                crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            }
        }
        !crc
    }

    pub fn write_stored_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut output = Vec::new();
        let mut central = Vec::new();
        for (name, data) in entries {
            let name = name.as_bytes();
            let offset = u32::try_from(output.len()).unwrap();
            let size = u32::try_from(data.len()).unwrap();
            let name_len = u16::try_from(name.len()).unwrap();
            let crc = crc32(data);

            output.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
            output.extend_from_slice(&20_u16.to_le_bytes());
            output.extend_from_slice(&0_u16.to_le_bytes());
            output.extend_from_slice(&0_u16.to_le_bytes());
            output.extend_from_slice(&0_u16.to_le_bytes());
            output.extend_from_slice(&0_u16.to_le_bytes());
            output.extend_from_slice(&crc.to_le_bytes());
            output.extend_from_slice(&size.to_le_bytes());
            output.extend_from_slice(&size.to_le_bytes());
            output.extend_from_slice(&name_len.to_le_bytes());
            output.extend_from_slice(&0_u16.to_le_bytes());
            output.extend_from_slice(name);
            output.extend_from_slice(data);

            central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&name_len.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u32.to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name);
        }
        let central_offset = u32::try_from(output.len()).unwrap();
        let central_size = u32::try_from(central.len()).unwrap();
        output.extend_from_slice(&central);
        output.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        let count = u16::try_from(entries.len()).unwrap();
        output.extend_from_slice(&count.to_le_bytes());
        output.extend_from_slice(&count.to_le_bytes());
        output.extend_from_slice(&central_size.to_le_bytes());
        output.extend_from_slice(&central_offset.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        std::fs::write(path, output).unwrap();
    }

    fn write_stored_gzip(path: &Path, data: &[u8]) {
        assert!(data.len() <= usize::from(u16::MAX));
        let length = u16::try_from(data.len()).unwrap();
        let mut output = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255, 1];
        output.extend_from_slice(&length.to_le_bytes());
        output.extend_from_slice(&(!length).to_le_bytes());
        output.extend_from_slice(data);
        output.extend_from_slice(&crc32(data).to_le_bytes());
        output.extend_from_slice(&u32::from(length).to_le_bytes());
        std::fs::write(path, output).unwrap();
    }

    #[test]
    fn archive_extensions_match_cog() {
        for extension in [
            "zip", "rar", "7z", "rsn", "vgm7z", "gz", "mdz", "mdr", "s3z", "xmz", "itz", "mptmz",
            "ZIP",
        ] {
            assert!(is_path(Path::new(&format!("music.{extension}"))));
        }
        assert!(!is_path(Path::new("music.tar")));
    }

    pub fn tree_url(archive: &Path, entry: &str, directory: bool) -> PathBuf {
        let mut url = url::Url::parse("kog-archive:").unwrap();
        url.query_pairs_mut()
            .append_pair("archive", archive.to_str().unwrap())
            .append_pair("entry", entry)
            .append_pair("directory", if directory { "1" } else { "0" });
        PathBuf::from(url.as_str())
    }

    #[test]
    fn tree_archive_selection_keeps_identity_companions_and_shared_workspace() {
        let fixture = tempfile::tempdir().unwrap();
        let archive = fixture.path().join("Set + 日本語.zip");
        let wav = wav_bytes(100);
        write_stored_zip(
            &archive,
            &[
                ("Disc/b + #%.wav", &wav),
                ("Disc/a.wav", &wav),
                ("Disc/companion.txt", b"keep me"),
                ("Other/c.wav", &wav),
            ],
        );
        let registry = DecoderRegistry::default();
        let selected = tree_url(&archive, "Disc/b + #%.wav", false);
        assert_eq!(
            tree_location(&selected).unwrap().unwrap().entry,
            "Disc/b + #%.wav"
        );
        let one = registry.expand_detailed(selected).unwrap();
        assert_eq!(one.sources.len(), 1);
        let source = &one.sources[0];
        assert_eq!(
            source.archive_origin.as_ref().unwrap().entry_name,
            "Disc/b + #%.wav"
        );
        assert!(
            source
                .path
                .parent()
                .unwrap()
                .join("companion.txt")
                .is_file()
        );
        assert!(registry.probe(source).is_ok());
        let worker = registry.background_worker(DecoderSettings::default());
        let folder = worker
            .expand_detailed(tree_url(&archive, "Disc", true))
            .unwrap();
        assert_eq!(folder.sources.len(), 2);
        assert_eq!(
            folder.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "Disc/a.wav"
        );
        assert_eq!(
            folder.sources[1].path, source.path,
            "Selections reuse one extracted workspace"
        );
        drop(worker);
        assert!(
            source.path.is_file(),
            "Queued tracks outlive the import worker"
        );
        assert!(
            registry
                .expand_detailed(tree_url(&archive, "Dis", true))
                .is_err(),
            "Directory selection uses a path boundary"
        );
        for name in ["../outside.wav", "/absolute.wav", "C:/absolute.wav"] {
            assert!(tree_location(&tree_url(&archive, name, false)).is_err());
        }
    }

    #[test]
    fn path_sanitizer_normalizes_separators_and_rejects_escape() {
        assert_eq!(
            safe_relative_path("album\\disc/song.vgm").unwrap(),
            PathBuf::from("album/disc/song.vgm")
        );
        for unsafe_path in ["../song.vgm", "/tmp/song.vgm", "C:\\song.vgm", "."] {
            assert!(safe_relative_path(unsafe_path).is_err(), "{unsafe_path}");
        }
    }

    #[test]
    fn metadata_archive_entries_are_not_extracted() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("metadata.zip");
        write_stored_zip(
            &path,
            &[
                ("._song.flac", b"junk"),
                ("__MACOSX/Album/song.flac", b"junk"),
                ("Album/desktop.ini", b"junk"),
                ("Album/.real.flac", b"music"),
            ],
        );
        let extracted = ExtractedArchive::open(&path).unwrap();
        assert_eq!(extracted.entries.len(), 1);
        assert_eq!(extracted.entries[0].name, "Album/.real.flac");
        assert!(!extracted.root().join("__MACOSX").exists());
    }

    #[test]
    fn zip_expands_playable_entries_in_order_and_keeps_logical_identity() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("album.zip");
        let first = wav_bytes(-2_000);
        let second = wav_bytes(1_000);
        write_stored_zip(
            &archive_path,
            &[
                ("notes.txt", b"not audio"),
                ("disc\\first.wav", &first),
                ("../escape.wav", &first),
                ("second.wav", &second),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand ZIP archive");
        assert_eq!(expansion.sources.len(), 2);
        assert_eq!(expansion.warnings.len(), 1);
        assert!(expansion.warnings[0].contains("parent traversal"));
        assert_eq!(
            expansion.sources[0].archive_origin,
            Some(ArchiveOrigin {
                archive_path: archive_path.canonicalize().unwrap(),
                entry_name: "disc/first.wav".to_owned(),
            })
        );
        assert_eq!(
            expansion.sources[1]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "second.wav"
        );
        assert!(expansion.sources.iter().all(|source| source.path.is_file()));
        assert_eq!(
            registry.probe(&expansion.sources[0]).unwrap().duration,
            Some(std::time::Duration::from_millis(10))
        );
        assert_eq!(
            expansion.sources[0].display_label(),
            format!("{} :: disc/first.wav", archive_path.display())
        );

        let same_logical_source = PlaybackSource {
            path: PathBuf::from("/different/temporary/path.wav"),
            remote_url: None,
            subsong: None,
            archive_origin: expansion.sources[0].archive_origin.clone(),
        };
        assert_eq!(expansion.sources[0], same_logical_source);
    }

    #[test]
    fn zip_expands_syntrax_subsongs_and_keeps_logical_identity() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("syntrax-set.zip");
        let jxs = test_jxs_bytes();
        write_stored_zip(&archive_path, &[("set/song.jxs", &jxs)]);

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived JXS");
        assert_eq!(expansion.sources.len(), 2);
        assert_eq!(expansion.sources[0].subsong, Some(0));
        assert_eq!(expansion.sources[1].subsong, Some(1));
        assert_eq!(
            expansion.sources[1]
                .archive_origin
                .as_ref()
                .expect("archived JXS identity")
                .entry_name,
            "set/song.jxs"
        );
        let properties = registry
            .probe(&expansion.sources[1])
            .expect("probe archived JXS subsong");
        assert_eq!(properties.title.as_deref(), Some("Synthetic JXS B"));
        assert_eq!(properties.track_number, Some(2));

        let playlist_path = fixture.path().join("saved-selection.m3u");
        Playlist::save(
            &playlist_path,
            &[PlaylistEntry {
                location: PlaylistLocation::Archive {
                    archive_path: archive_path.canonicalize().unwrap(),
                    entry_name: "set/song.jxs".to_owned(),
                },
                fragment: Some("1".to_owned()),
            }],
        )
        .expect("save archived JXS selection");
        let restored = registry
            .expand_detailed(playlist_path)
            .expect("reopen archived JXS selection");
        assert_eq!(restored.sources.len(), 1);
        assert_eq!(restored.sources[0].subsong, Some(1));
        assert_eq!(
            restored.sources[0]
                .archive_origin
                .as_ref()
                .expect("restored archive identity")
                .entry_name,
            "set/song.jxs"
        );
        assert_eq!(
            registry
                .probe(&restored.sources[0])
                .expect("probe restored archived JXS")
                .title
                .as_deref(),
            Some("Synthetic JXS B")
        );
    }

    #[test]
    fn zip_expands_format_two_midi_tracks_with_stable_archive_identity() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("midi-set.zip");
        let midi = format_two_midi_bytes();
        write_stored_zip(&archive_path, &[("set/two-songs.mid", &midi)]);

        let registry = DecoderRegistry::new(DecoderSettings::new(None, MidiEngine::Opl3Windows));
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived format 2 MIDI");
        assert!(expansion.warnings.is_empty());
        assert_eq!(expansion.sources.len(), 2);
        assert_eq!(expansion.sources[0].subsong, Some(0));
        assert_eq!(expansion.sources[1].subsong, Some(1));
        assert!(expansion.sources.iter().all(|source| {
            source
                .archive_origin
                .as_ref()
                .is_some_and(|origin| origin.entry_name == "set/two-songs.mid")
        }));
        assert!(expansion.sources[0].display_label().ends_with("#1"));
        assert!(expansion.sources[1].display_label().ends_with("#2"));

        let first = registry
            .probe(&expansion.sources[0])
            .expect("probe first archived MIDI song");
        assert_eq!(first.title.as_deref(), Some("Archive Opening"));
        assert_eq!(first.duration, Some(std::time::Duration::from_millis(500)));
        let second = registry
            .probe(&expansion.sources[1])
            .expect("probe second archived MIDI song");
        assert_eq!(second.title.as_deref(), Some("Archive Finale"));
        assert_eq!(second.duration, Some(std::time::Duration::from_secs(1)));

        let playlist_path = fixture.path().join("selected.m3u");
        Playlist::save(
            &playlist_path,
            &[PlaylistEntry {
                location: PlaylistLocation::Archive {
                    archive_path: archive_path.canonicalize().unwrap(),
                    entry_name: "set/two-songs.mid".to_owned(),
                },
                fragment: Some("1".to_owned()),
            }],
        )
        .expect("save selected archived MIDI track");
        let restored = registry
            .expand_detailed(playlist_path)
            .expect("restore selected archived MIDI track");
        assert_eq!(restored.sources.len(), 1);
        assert_eq!(restored.sources[0].subsong, Some(1));
        assert_eq!(
            registry
                .probe(&restored.sources[0])
                .expect("probe restored archived MIDI track")
                .title
                .as_deref(),
            Some("Archive Finale")
        );
    }

    #[test]
    fn twenty_midi_zip_probes_without_starting_sc55_helpers() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("twenty-midis.zip");
        let midi = vec![
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 1, 0xe0, b'M', b'T', b'r', b'k', 0, 0,
            0, 16, 0, 0xc0, 0, 0, 0x90, 60, 100, 0x83, 0x60, 0x80, 60, 64, 0, 0xff, 0x2f, 0,
        ];
        let names = (1..=20)
            .map(|index| format!("album/{index:03}.mid"))
            .collect::<Vec<_>>();
        let entries = names
            .iter()
            .map(|name| (name.as_str(), midi.as_slice()))
            .collect::<Vec<_>>();
        write_stored_zip(&archive_path, &entries);

        // The directory intentionally has no ROMs. Playlist metadata should
        // parse all twenty MIDI timelines without constructing the emulator.
        let settings = DecoderSettings::new(None, MidiEngine::Sc55)
            .with_sc55_rom_path(Some(fixture.path().to_owned()));
        let registry = DecoderRegistry::new(settings);
        let expansion = registry
            .expand_detailed(archive_path)
            .expect("expand twenty-file MIDI archive");
        assert_eq!(expansion.sources.len(), 20);
        for source in &expansion.sources {
            let properties = registry
                .probe(source)
                .expect("probe archived MIDI without starting SC-55");
            assert_eq!(
                properties.duration,
                Some(std::time::Duration::from_millis(500))
            );
            assert_eq!(properties.codec.as_deref(), Some("Nuked SC-55"));
        }
    }

    #[test]
    fn gzip_uses_the_outer_filename_and_decodes_end_to_end() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("single.wav.gz");
        write_stored_gzip(&archive_path, &wav_bytes(-1_000));

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand GZip stream");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "single.wav"
        );
        assert_eq!(
            registry.probe(&expansion.sources[0]).unwrap().duration,
            Some(std::time::Duration::from_millis(10))
        );
    }

    #[test]
    fn extracted_tree_preserves_relative_companion_resolution() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("linked.zip");
        let apl = concat!(
            "[Monkey's Audio Image Link File]\r\n",
            "Image File=image.wav\r\n",
            "Start Block=20\r\n",
            "Finish Block=60\r\n",
            "----- APE TAG (DO NOT TOUCH!!!) -----\r\n",
        );
        write_stored_zip(
            &archive_path,
            &[
                ("disc/selection.apl", apl.as_bytes()),
                ("disc/image.wav", &wav_bytes(-1_000)),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path)
            .expect("expand linked archive");
        assert_eq!(expansion.sources.len(), 2);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "disc/selection.apl"
        );
        let properties = registry
            .probe(&expansion.sources[0])
            .expect("probe APL through extracted companion");
        assert_eq!(
            properties.duration,
            Some(std::time::Duration::from_millis(5))
        );
        assert_eq!(properties.sample_rate, Some(8_000));
    }

    #[test]
    fn zip_preserves_minincsf_library_resolution() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("ncsf-set.zip");
        let library = test_ncsf_bytes(Some(&test_sdat_bytes()), "title=Library\n");
        let mini = test_ncsf_bytes(
            None,
            "_lib=music.ncsflib\ntitle=Archive selection\nlength=0:00.250\n",
        );
        write_stored_zip(
            &archive_path,
            &[
                ("set/selection.minincsf", &mini),
                ("set/music.ncsflib", &library),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived NCSF set");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "set/selection.minincsf"
        );
        let properties = registry
            .probe(&expansion.sources[0])
            .expect("probe archived minincsf through extracted library");
        assert_eq!(properties.title.as_deref(), Some("Archive selection"));
        assert_eq!(
            properties.duration,
            Some(std::time::Duration::from_millis(250))
        );
    }

    #[test]
    fn zip_preserves_minigsf_library_resolution() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("gsf-set.zip");
        let library = test_gsf_bytes(Some(&test_gba_rom()), "title=Library\n");
        let mini = test_gsf_bytes(
            None,
            "_lib=music.gsflib\ntitle=Archive selection\nlength=0:00.250\n",
        );
        write_stored_zip(
            &archive_path,
            &[
                ("set/selection.minigsf", &mini),
                ("set/music.gsflib", &library),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived GSF set");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "set/selection.minigsf"
        );
        let properties = registry
            .probe(&expansion.sources[0])
            .expect("probe archived minigsf through extracted library");
        assert_eq!(properties.title.as_deref(), Some("Archive selection"));
        assert_eq!(
            properties.duration,
            Some(std::time::Duration::from_millis(250))
        );
    }

    #[test]
    fn zip_preserves_miniqsf_library_resolution() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("qsf-set.zip");
        let library = test_qsf_bytes(Some(&test_qsf_program()), "title=Library\n");
        let mini = test_qsf_bytes(
            None,
            "_lib=music.qsflib\ntitle=Archive selection\nlength=0:00.250\n",
        );
        write_stored_zip(
            &archive_path,
            &[
                ("set/selection.miniqsf", &mini),
                ("set/music.qsflib", &library),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived QSF set");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "set/selection.miniqsf"
        );
        let properties = registry
            .probe(&expansion.sources[0])
            .expect("probe archived miniqsf through extracted library");
        assert_eq!(properties.title.as_deref(), Some("Archive selection"));
        let duration = properties.duration.expect("archived QSF duration");
        let expected = std::time::Duration::from_millis(250);
        let frame = std::time::Duration::from_nanos(1_000_000_000 / 24_038 + 1);
        assert!(duration.abs_diff(expected) <= frame);
    }

    #[test]
    fn zip_preserves_minissf_library_resolution() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("ssf-set.zip");
        let library = test_sdsf_bytes(0x11, Some(&test_ssf_program()), "title=Library\n");
        let mini = test_sdsf_bytes(
            0x11,
            None,
            "_lib=music.ssflib\ntitle=Archive selection\nlength=0:00.250\n",
        );
        write_stored_zip(
            &archive_path,
            &[
                ("set/selection.minissf", &mini),
                ("set/music.ssflib", &library),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived SSF set");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "set/selection.minissf"
        );
        let properties = registry
            .probe(&expansion.sources[0])
            .expect("probe archived minissf through extracted library");
        assert_eq!(properties.title.as_deref(), Some("Archive selection"));
        let duration = properties.duration.expect("archived SSF duration");
        let expected = std::time::Duration::from_millis(250);
        let frame = std::time::Duration::from_nanos(1_000_000_000 / 44_100 + 1);
        assert!(duration.abs_diff(expected) <= frame);
    }

    #[test]
    fn zip_preserves_miniusf_library_resolution() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("usf-set.zip");
        let library = test_usf_bytes(Some(&test_usf_reserved()), "title=Library\n");
        let mini = test_usf_bytes(
            None,
            "_lib=music.usflib\ntitle=Archive selection\nlength=0:00.250\n",
        );
        write_stored_zip(
            &archive_path,
            &[
                ("set/selection.miniusf", &mini),
                ("set/music.usflib", &library),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived USF set");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "set/selection.miniusf"
        );
        let properties = registry
            .probe(&expansion.sources[0])
            .expect("probe archived miniusf through extracted library");
        assert_eq!(properties.title.as_deref(), Some("Archive selection"));
        let duration = properties.duration.expect("archived USF duration");
        let expected = std::time::Duration::from_millis(250);
        let frame = std::time::Duration::from_nanos(1_000_000_000 / 44_100 + 1);
        assert!(duration.abs_diff(expected) <= frame);
    }

    #[test]
    fn zip_preserves_minipsf_library_resolution() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("psf-set.zip");
        let library = test_psf_bytes(Some(&test_psf_executable()), "title=Library\n");
        let mini = test_psf_bytes(
            None,
            "_lib=music.psflib\ntitle=Archive selection\nlength=0:00.250\n",
        );
        write_stored_zip(
            &archive_path,
            &[
                ("set/selection.minipsf", &mini),
                ("set/music.psflib", &library),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived PSF set");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "set/selection.minipsf"
        );
        let properties = registry
            .probe(&expansion.sources[0])
            .expect("probe archived minipsf through extracted library");
        assert_eq!(properties.title.as_deref(), Some("Archive selection"));
        let duration = properties.duration.expect("archived PSF duration");
        let expected = std::time::Duration::from_millis(250);
        let frame = std::time::Duration::from_nanos(1_000_000_000 / 44_100 + 1);
        assert!(duration.abs_diff(expected) <= frame);
    }

    #[test]
    fn zip_preserves_minipsf2_library_resolution() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("psf2-set.zip");
        let irx = test_psf2_irx();
        let library = test_psf2_bytes(&[("psf2.irx", &irx)], "title=Library\n");
        let mini = test_psf2_bytes(
            &[],
            "_lib=music.psflib2\ntitle=Archive PSF2 selection\nlength=0:00.250\n",
        );
        write_stored_zip(
            &archive_path,
            &[
                ("set/selection.minipsf2", &mini),
                ("set/music.psflib2", &library),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived PSF2 set");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "set/selection.minipsf2"
        );
        let properties = registry
            .probe(&expansion.sources[0])
            .expect("probe archived miniPSF2 through extracted library");
        assert_eq!(properties.title.as_deref(), Some("Archive PSF2 selection"));
        let duration = properties.duration.expect("archived PSF2 duration");
        let expected = std::time::Duration::from_millis(250);
        let frame = std::time::Duration::from_nanos(1_000_000_000 / 44_100 + 1);
        assert!(duration.abs_diff(expected) <= frame);
    }

    #[test]
    fn zip_preserves_minisnsf_library_resolution() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("snsf-set.zip");
        let library = test_snsf_bytes(0, &test_snsf_rom(), &[], "title=Library\n");
        let mini = test_snsf_bytes(
            0,
            &[],
            &[],
            "_lib=music.snsflib\ntitle=Archive SNSF selection\nlength=0:00.250\n",
        );
        write_stored_zip(
            &archive_path,
            &[
                ("set/selection.minisnsf", &mini),
                ("set/music.snsflib", &library),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived SNSF set");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "set/selection.minisnsf"
        );
        let properties = registry
            .probe(&expansion.sources[0])
            .expect("probe archived miniSNSF through extracted library");
        assert_eq!(properties.title.as_deref(), Some("Archive SNSF selection"));
        let duration = properties.duration.expect("archived SNSF duration");
        let expected = std::time::Duration::from_millis(250);
        let frame = std::time::Duration::from_nanos(1_000_000_000 / 32_000 + 1);
        assert!(duration.abs_diff(expected) <= frame);
    }

    #[test]
    fn zip_preserves_mini2sf_library_resolution() {
        let fixture = tempfile::tempdir().unwrap();
        let archive_path = fixture.path().join("twosf-set.zip");
        let library = test_twosf_bytes(0, &test_twosf_rom(), "title=Library\n");
        let mini = test_twosf_bytes(
            0,
            &[],
            "_lib=music.2sflib\ntitle=Archive 2SF selection\nlength=0:00.250\n",
        );
        write_stored_zip(
            &archive_path,
            &[
                ("set/selection.mini2sf", &mini),
                ("set/music.2sflib", &library),
            ],
        );

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(archive_path.clone())
            .expect("expand archived 2SF set");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0]
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "set/selection.mini2sf"
        );
        let properties = registry
            .probe(&expansion.sources[0])
            .expect("probe archived mini2SF through extracted library");
        assert_eq!(properties.title.as_deref(), Some("Archive 2SF selection"));
        let duration = properties.duration.expect("archived 2SF duration");
        let expected = std::time::Duration::from_millis(250);
        let frame = std::time::Duration::from_nanos(1_000_000_000 / 32_728 + 1);
        assert!(duration.abs_diff(expected) <= frame);
    }

    #[test]
    fn seven_zip_rar_and_cog_aliases_use_real_format_detection() {
        let fixture = tempfile::tempdir().unwrap();
        for extension in ["7z", "vgm7z"] {
            let path = fixture.path().join(format!("music.{extension}"));
            std::fs::write(&path, EMPTY_7Z).unwrap();
            let extracted = ExtractedArchive::open(&path).expect("extract 7Z family");
            assert_eq!(extracted.entries.len(), 1);
            assert_eq!(extracted.entries[0].name, "empty.txt");
            assert_eq!(std::fs::read(&extracted.entries[0].path).unwrap(), b"");
        }

        for extension in ["rar", "rsn"] {
            let path = fixture.path().join(format!("music.{extension}"));
            std::fs::write(&path, STORED_RAR5).unwrap();
            let extracted = ExtractedArchive::open(&path).expect("extract RAR family");
            assert_eq!(extracted.entries.len(), 1);
            assert_eq!(extracted.entries[0].name, "helloworld.txt");
            assert_eq!(
                std::fs::read(&extracted.entries[0].path).unwrap(),
                b"hello libarchive test suite!\n"
            );
        }
    }

    fn stored_zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pack.zip");
        write_stored_zip(&path, entries);
        std::fs::read(&path).unwrap()
    }

    #[test]
    fn zip_central_directory_names_match_archive_reader() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("music.zip");
        write_stored_zip(
            &path,
            &[
                ("Disc/Opening.mid", b"one"),
                ("Disc/Café.mid", b"two"),
                ("Other/", b""),
            ],
        );
        let old_names = compress_tools::list_archive_files_with_encoding(
            File::open(&path).unwrap(),
            decode_archive_name,
        )
        .unwrap();
        assert_eq!(list_archive_names(&path).unwrap(), old_names);
    }

    #[test]
    fn archive_name_cache_reuses_and_invalidates_listings() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("changing.zip");
        write_stored_zip(&path, &[("first.mid", b"one")]);
        let first = list_archive_names_shared(&path).unwrap();
        let again = list_archive_names_shared(&path).unwrap();
        assert!(Arc::ptr_eq(&first, &again));
        write_stored_zip(&path, &[("different-title.mid", b"two")]);
        let changed = list_archive_names_shared(&path).unwrap();
        assert_eq!(changed.as_slice(), &["different-title.mid"]);
        assert!(!Arc::ptr_eq(&first, &changed));
    }

    #[test]
    fn member_url_round_trips_through_tree_location() {
        let url = member_url(Path::new("/music/pack.zip"), "Disc/a.wav", false);
        let location = tree_location(&url)
            .expect("parse member url")
            .expect("location");
        assert_eq!(location.archive, PathBuf::from("/music/pack.zip"));
        assert_eq!(location.entry, "Disc/a.wav");
        assert!(!location.directory);
        let dir_url = member_url(Path::new("/music/pack.zip"), "Disc", true);
        let dir_location = tree_location(&dir_url)
            .expect("parse dir url")
            .expect("location");
        assert!(dir_location.directory);
    }

    struct TestCacheDir {
        _guard: std::sync::MutexGuard<'static, ()>,
    }
    impl TestCacheDir {
        fn set(path: &Path) -> Self {
            // Serialized: parallel tests share one process-wide variable.
            // SAFETY: only nested tests touch KOG_NESTED_CACHE_DIR, and
            // every other nested test passes its cache explicitly.
            static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            unsafe { std::env::set_var("KOG_NESTED_CACHE_DIR", path); }
            Self { _guard: guard }
        }
    }
    impl Drop for TestCacheDir {
        fn drop(&mut self) {
            // SAFETY: paired with the scoped set above.
            unsafe { std::env::remove_var("KOG_NESTED_CACHE_DIR"); }
        }
    }

    #[test]
    fn nested_chain_materializes_reuses_and_detects_staleness() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        let inner = stored_zip_bytes(&[("song.wav", &wav_bytes(100))]);
        let outer = root.join("outer.zip");
        write_stored_zip(&outer, &[("inner.zip", &inner), ("plain.txt", b"")]);
        let cache = root.join("nested-cache");

        let resolved =
            resolve_archive_chain(&outer, "inner.zip/song.wav", &cache).expect("resolve nested");
        assert_eq!(resolved.leaf, "song.wav");
        assert_eq!(resolved.prefix, "inner.zip");
        assert_eq!(resolved.links, 1);
        assert_eq!(
            resolved.container.extension().and_then(|ext| ext.to_str()),
            Some("zip")
        );
        assert!(resolved.container.starts_with(&cache));
        assert_eq!(std::fs::read(&resolved.container).unwrap(), inner);

        let reused =
            resolve_archive_chain(&outer, "inner.zip/song.wav", &cache).expect("reuse cache");
        assert_eq!(resolved.container, reused.container);

        let flat =
            resolve_archive_chain(&outer, "plain.txt", &cache).expect("plain member");
        assert_eq!(flat.leaf, "plain.txt");
        assert_eq!(flat.links, 0);
        assert_eq!(
            flat.container,
            outer.canonicalize().unwrap(),
            "non-nested entries resolve unchanged"
        );

        let missing = resolve_archive_chain(&outer, "missing/track.wav", &cache)
            .expect("missing entries resolve unchanged");
        assert_eq!(missing.leaf, "missing/track.wav");
        assert_eq!(missing.container, outer.canonicalize().unwrap());

        write_stored_zip(
            &outer,
            &[("inner.zip", &inner), ("plain.txt", b""), ("extra.txt", b"x")],
        );
        let renewed =
            resolve_archive_chain(&outer, "inner.zip/song.wav", &cache).expect("re-resolve");
        assert_ne!(
            resolved.container, renewed.container,
            "changed outer archives invalidate the cache"
        );
    }

    #[test]
    fn nested_chain_prefers_explicit_directories() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        // "pack.zip/" only exists as an implied directory here, so the plain
        // file "pack.zip/x.flac" must resolve without descending.
        let outer = root.join("outer.zip");
        write_stored_zip(&outer, &[("pack.zip/x.flac", &wav_bytes(100))]);
        let cache = root.join("nested-cache");
        let resolved = resolve_archive_chain(&outer, "pack.zip/x.flac", &cache)
            .expect("implied directories never descend");
        assert_eq!(resolved.leaf, "pack.zip/x.flac");
        assert_eq!(resolved.links, 0);
        assert_eq!(resolved.container, outer.canonicalize().unwrap());
    }

    #[test]
    fn nested_chain_depth_cap_rejects_runaway_nesting() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        let mut payload = stored_zip_bytes(&[("deep.wav", &wav_bytes(100))]);
        for level in ["l5.zip", "l4.zip", "l3.zip", "l2.zip", "l1.zip"] {
            payload = stored_zip_bytes(&[(level, &payload)]);
        }
        let outer = root.join("outer.zip");
        write_stored_zip(&outer, &[("l0.zip", &payload)]);
        let cache = root.join("nested-cache");
        let error = resolve_archive_chain(
            &outer,
            "l0.zip/l1.zip/l2.zip/l3.zip/l4.zip/l5.zip/deep.wav",
            &cache,
        )
        .unwrap_err();
        assert!(
            error.contains("4-level"),
            "runaway nesting fails loudly: {error}"
        );
    }

    #[test]
    fn whole_archive_expansion_includes_nested_tracks() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        let _cache_guard = TestCacheDir::set(&root.join("nested-cache"));
        let inner = stored_zip_bytes(&[("deep.wav", &wav_bytes(100))]);
        let outer = root.join("outer.zip");
        write_stored_zip(
            &outer,
            &[("inner.zip", &inner), ("top.wav", &wav_bytes(100))],
        );
        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(outer.clone())
            .expect("expand outer archive with a nested member");
        assert_eq!(expansion.sources.len(), 2);
        assert!(
            expansion.warnings.iter().all(|warning| !warning.contains("Nested archive")),
            "no nested warnings: {:?}",
            expansion.warnings
        );
        let deep = expansion
            .sources
            .iter()
            .find(|source| {
                source.path.file_name().and_then(|name| name.to_str()) == Some("deep.wav")
            })
            .expect("nested track expanded");
        let origin = deep.archive_origin.as_ref().expect("nested chain origin");
        assert_eq!(origin.entry_name, "inner.zip/deep.wav");
        assert_eq!(origin.archive_path, outer.canonicalize().unwrap());
    }

    #[test]
    fn nested_archive_expands_to_outer_chain_origin() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        let cache = root.join("nested-cache");
        let _cache_guard = TestCacheDir::set(&cache);
        let inner = stored_zip_bytes(&[("song.wav", &wav_bytes(100))]);
        let outer = root.join("outer.zip");
        write_stored_zip(&outer, &[("inner.zip", &inner)]);
        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(tree_url(&outer, "inner.zip/song.wav", false))
            .expect("expand nested archive member");
        assert_eq!(expansion.sources.len(), 1);
        let origin = expansion.sources[0]
            .archive_origin
            .as_ref()
            .expect("nested track keeps a stable origin");
        assert_eq!(origin.entry_name, "inner.zip/song.wav");
        assert_eq!(
            origin.archive_path,
            outer.canonicalize().unwrap(),
            "nested origin names the real outer archive, not a cache copy"
        );
        assert!(std::fs::read(&expansion.sources[0].path).is_ok());
    }
}
