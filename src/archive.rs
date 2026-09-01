//! Safe temporary extraction for Cog-compatible audio archives.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use compress_tools::{ArchiveContents, ArchiveIteratorBuilder};
use encoding_rs::{GB18030, WINDOWS_1251};
use tempfile::TempDir;

const MAX_ENTRIES: usize = 16_384;
const MAX_ENTRY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const FILE_TYPE_MASK: u32 = 0o170_000;
const FILE_TYPE_DIRECTORY: u32 = 0o040_000;
const FILE_TYPE_REGULAR: u32 = 0o100_000;

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

impl ExtractedArchive {
    pub fn open(path: &Path) -> Result<Self, String> {
        if !is_path(path) {
            return Err(format!(
                "{} is not a supported audio archive",
                path.display()
            ));
        }

        let source = File::open(path)
            .map_err(|error| format!("opening archive {}: {error}", path.display()))?;
        let temporary_directory = tempfile::Builder::new()
            .prefix("kog-archive-")
            .tempdir()
            .map_err(|error| format!("creating archive workspace: {error}"))?;
        let raw_gzip = extension(path).is_some_and(|value| value.eq_ignore_ascii_case("gz"));
        let iterator = ArchiveIteratorBuilder::new(source)
            .decoder(decode_archive_name)
            .mtree_format(false)
            .raw_format(raw_gzip)
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
                    if entry_count > MAX_ENTRIES {
                        return Err(format!(
                            "archive {} exceeds Kog's {MAX_ENTRIES}-entry safety limit",
                            path.display()
                        ));
                    }

                    let name = raw_entry_name(path, &name, raw_gzip);
                    let relative = match safe_relative_path(&name) {
                        Ok(relative) => relative,
                        Err(error) => {
                            warnings.push(format!("Skipped archive entry {name:?}: {error}"));
                            current = CurrentEntry::Discard { written: 0 };
                            continue;
                        }
                    };
                    if !seen_paths.insert(relative.clone()) {
                        warnings.push(format!(
                            "Skipped duplicate archive entry {}",
                            portable_name(&relative)
                        ));
                        current = CurrentEntry::Discard { written: 0 };
                        continue;
                    }

                    let mode = stat.st_mode & FILE_TYPE_MASK;
                    let named_directory = name.ends_with('/') || name.ends_with('\\');
                    let is_directory = mode == FILE_TYPE_DIRECTORY || named_directory;
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
                    if stat.st_size > 0 && stat.st_size as u64 > MAX_ENTRY_BYTES {
                        return Err(format!(
                            "archive entry {} exceeds Kog's {} GiB per-file safety limit",
                            portable_name(&relative),
                            MAX_ENTRY_BYTES / 1024 / 1024 / 1024
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
                    if total_bytes > MAX_TOTAL_BYTES {
                        return Err(format!(
                            "archive {} exceeds Kog's {} GiB expanded-size safety limit",
                            path.display(),
                            MAX_TOTAL_BYTES / 1024 / 1024 / 1024
                        ));
                    }
                    match &mut current {
                        CurrentEntry::File { file, written } => {
                            *written = written
                                .checked_add(chunk_size)
                                .ok_or_else(|| "archive entry size overflowed".to_owned())?;
                            if *written > MAX_ENTRY_BYTES {
                                return Err(format!(
                                    "an entry in {} exceeds Kog's {} GiB per-file safety limit",
                                    path.display(),
                                    MAX_ENTRY_BYTES / 1024 / 1024 / 1024
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
        ["zip", "rar", "7z", "rsn", "vgm7z", "gz"]
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(extension))
    })
}

fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|value| value.to_str())
}

fn raw_entry_name(archive: &Path, name: &str, raw_gzip: bool) -> String {
    if !raw_gzip || name != "data" {
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
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.to_owned());
    }
    let (text, _, errors) = GB18030.decode(bytes);
    if !errors {
        return Ok(text.into_owned());
    }
    let (text, _, errors) = WINDOWS_1251.decode(bytes);
    if !errors {
        return Ok(text.into_owned());
    }
    Ok(bytes.iter().map(|byte| char::from(*byte)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{ArchiveOrigin, DecoderRegistry, DecoderSettings, PlaybackSource};
    use crate::gsf::{test_gba_rom, test_gsf_bytes};
    use crate::ncsf::{test_ncsf_bytes, test_sdat_bytes};
    use crate::psf::{test_psf_bytes, test_psf_executable, test_psf2_bytes, test_psf2_irx};
    use crate::qsf::{test_qsf_bytes, test_qsf_program};
    use crate::sdsf::{test_sdsf_bytes, test_ssf_program};
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

    fn wav_bytes(seed: i16) -> Vec<u8> {
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

    fn write_stored_zip(path: &Path, entries: &[(&str, &[u8])]) {
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
        for extension in ["zip", "rar", "7z", "rsn", "vgm7z", "gz", "ZIP"] {
            assert!(is_path(Path::new(&format!("music.{extension}"))));
        }
        assert!(!is_path(Path::new("music.tar")));
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
            subsong: None,
            archive_origin: expansion.sources[0].archive_origin.clone(),
        };
        assert_eq!(expansion.sources[0], same_logical_source);
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
}
