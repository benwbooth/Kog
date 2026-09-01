//! Safe import of compressed user-supplied synthesizer ROM sets.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::archive::ExtractedArchive;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RomKind {
    Sc55,
    Mt32,
}

impl RomKind {
    const fn directory_name(self) -> &'static str {
        match self {
            Self::Sc55 => "sc55",
            Self::Mt32 => "mt32",
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::Sc55 => "SC-55",
            Self::Mt32 => "MT-32/CM-32L",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ImportedRomSet {
    pub directory: PathBuf,
    pub file_count: usize,
    pub warnings: Vec<String>,
}

pub fn import_rom_archive(path: &Path, kind: RomKind) -> Result<ImportedRomSet, String> {
    let project = ProjectDirs::from("org", "Kog", "Kog")
        .ok_or_else(|| "The platform data directory is unavailable".to_owned())?;
    let root = project
        .data_local_dir()
        .join("roms")
        .join(kind.directory_name());
    import_rom_archive_into(path, kind, &root)
}

fn import_rom_archive_into(
    path: &Path,
    kind: RomKind,
    root: &Path,
) -> Result<ImportedRomSet, String> {
    let path = std::fs::canonicalize(path)
        .map_err(|error| format!("opening ROM archive {}: {error}", path.display()))?;
    if !path.is_file() {
        return Err(format!("ROM archive is not a file: {}", path.display()));
    }
    let extracted = ExtractedArchive::open_rom(&path)?;
    if extracted.entries.is_empty() {
        return Err(format!(
            "ROM archive contains no regular files: {}",
            path.display()
        ));
    }

    std::fs::create_dir_all(root)
        .map_err(|error| format!("creating ROM storage {}: {error}", root.display()))?;
    let destination = tempfile::Builder::new()
        .prefix(&format!("{}-", kind.directory_name()))
        .tempdir_in(root)
        .map_err(|error| format!("creating {} ROM import: {error}", kind.display_name()))?;
    let mut names = HashSet::new();
    let mut file_count = 0_usize;

    for entry in &extracted.entries {
        let Some(file_name) = entry.path.file_name() else {
            continue;
        };
        let normalized_name = file_name.to_string_lossy().to_ascii_lowercase();
        if !names.insert(normalized_name) {
            return Err(format!(
                "ROM archive contains duplicate filename {} in different folders",
                file_name.to_string_lossy()
            ));
        }
        let metadata = std::fs::metadata(&entry.path)
            .map_err(|error| format!("reading imported ROM {}: {error}", entry.name))?;
        if metadata.len() == 0 {
            continue;
        }
        let target = destination.path().join(file_name);
        std::fs::copy(&entry.path, &target).map_err(|error| {
            format!(
                "copying imported ROM {} to {}: {error}",
                entry.name,
                target.display()
            )
        })?;
        file_count += 1;
    }
    if file_count == 0 {
        return Err(format!(
            "ROM archive contains no non-empty files: {}",
            path.display()
        ));
    }

    Ok(ImportedRomSet {
        directory: destination.keep(),
        file_count,
        warnings: extracted.warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn rom_import_detects_archives_by_content_and_flattens_nested_files() {
        let fixture = tempfile::tempdir().unwrap();
        let archive = fixture.path().join("roms.bundle");
        write_stored_zip(
            &archive,
            &[("model/control.rom", b"control"), ("model/pcm.rom", b"pcm")],
        );

        let imported =
            import_rom_archive_into(&archive, RomKind::Mt32, &fixture.path().join("imports"))
                .unwrap();
        assert_eq!(imported.file_count, 2);
        assert_eq!(
            std::fs::read(imported.directory.join("control.rom")).unwrap(),
            b"control"
        );
        assert_eq!(
            std::fs::read(imported.directory.join("pcm.rom")).unwrap(),
            b"pcm"
        );
    }

    #[test]
    fn rom_import_rejects_case_insensitive_flattening_collisions() {
        let fixture = tempfile::tempdir().unwrap();
        let archive = fixture.path().join("roms.zip");
        write_stored_zip(
            &archive,
            &[("one/CONTROL.ROM", b"one"), ("two/control.rom", b"two")],
        );

        let error =
            import_rom_archive_into(&archive, RomKind::Mt32, &fixture.path().join("imports"))
                .unwrap_err();
        assert!(error.contains("duplicate filename"));
    }
}
