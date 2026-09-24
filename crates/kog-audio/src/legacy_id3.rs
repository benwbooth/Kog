//! Conservative repair for ID3v2 text frames falsely labelled as Latin-1.
//! Lofty correctly follows the declared encoding, so the original bytes must
//! be read before they become mojibake. This only overrides strongly detected
//! non-Latin text; ordinary Latin-1 and Unicode frames remain Lofty's domain.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAX_TAG_BYTES: usize = 32 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 8192;

#[derive(Default)]
pub struct CorrectedId3Text {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub genre: Option<String>,
}

/// Avoid rereading normal tags (especially large embedded covers) during
/// background library scans. A few Western accents are not suspicious.
pub fn looks_misdecoded(values: &[&str]) -> bool {
    let mut high_latin = 0;
    for character in values.iter().flat_map(|value| value.chars()) {
        if character == '\u{fffd}' {
            return true;
        }
        if matches!(character, '\u{0080}'..='\u{00ff}') {
            high_latin += 1;
        }
    }
    high_latin >= 4
}

fn synchsafe(bytes: &[u8]) -> Option<usize> {
    (bytes.len() == 4 && bytes.iter().all(|byte| byte & 0x80 == 0)).then(|| {
        bytes
            .iter()
            .fold(0_usize, |size, byte| (size << 7) | usize::from(*byte))
    })
}

pub fn corrected_text(path: &Path) -> Option<CorrectedId3Text> {
    let mut file = File::open(path).ok()?;
    let mut header = [0_u8; 10];
    file.read_exact(&mut header).ok()?;
    if &header[..3] != b"ID3" || !matches!(header[3], 3 | 4) {
        return None;
    }
    // Unsynchronization and extended headers alter frame offsets. Let Lofty
    // handle those rather than guessing at raw byte boundaries.
    if header[5] & 0xc0 != 0 {
        return None;
    }
    let size = synchsafe(&header[6..10])?;
    if size == 0 || size > MAX_TAG_BYTES {
        return None;
    }
    let mut fields = Vec::<([u8; 4], Vec<u8>)>::new();
    let mut remaining = size;
    while remaining >= 10 {
        let mut frame = [0_u8; 10];
        file.read_exact(&mut frame).ok()?;
        remaining -= 10;
        let id: [u8; 4] = frame[..4].try_into().ok()?;
        if id == [0; 4] {
            break;
        }
        if !id
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            return None;
        }
        let frame_size = if header[3] == 3 {
            u32::from_be_bytes(frame[4..8].try_into().ok()?) as usize
        } else {
            synchsafe(&frame[4..8])?
        };
        if frame_size > remaining {
            return None;
        }
        let supported = matches!(
            &id,
            b"TIT2" | b"TPE1" | b"TALB" | b"TPE2" | b"TCOM" | b"TCON"
        );
        if supported && frame_size > 1 && frame_size <= MAX_TEXT_BYTES && frame[8..10] == [0, 0] {
            let mut payload = vec![0_u8; frame_size];
            file.read_exact(&mut payload).ok()?;
            if payload[0] == 0 {
                let text = payload[1..]
                    .split(|byte| *byte == 0)
                    .next()
                    .unwrap_or_default();
                if !text.is_empty() && !fields.iter().any(|(key, _)| key == &id) {
                    fields.push((id, text.to_vec()));
                }
            }
        } else {
            file.seek(SeekFrom::Current(frame_size.try_into().ok()?))
                .ok()?;
        }
        remaining -= frame_size;
    }
    let raw = fields
        .iter()
        .map(|(_, bytes)| bytes.as_slice())
        .collect::<Vec<_>>();
    let decoded = kog_core::text_encoding::decode_mislabelled_latin1_fields(&raw)?;
    let mut result = CorrectedId3Text::default();
    for ((id, _), text) in fields.into_iter().zip(decoded) {
        match &id {
            b"TIT2" => result.title = Some(text),
            b"TPE1" => result.artist = Some(text),
            b"TALB" => result.album = Some(text),
            b"TPE2" => result.album_artist = Some(text),
            b"TCOM" => result.composer = Some(text),
            b"TCON" => result.genre = Some(text),
            _ => {}
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(frames: &[(&[u8; 4], u8, &[u8])]) -> Vec<u8> {
        let mut body = Vec::new();
        for (id, encoding, text) in frames {
            body.extend_from_slice(*id);
            body.extend_from_slice(&((text.len() + 1) as u32).to_be_bytes());
            body.extend_from_slice(&[0, 0, *encoding]);
            body.extend_from_slice(text);
        }
        let size = body.len() as u32;
        let mut tag = b"ID3\x03\x00\x00".to_vec();
        tag.extend_from_slice(&[
            ((size >> 21) & 0x7f) as u8,
            ((size >> 14) & 0x7f) as u8,
            ((size >> 7) & 0x7f) as u8,
            (size & 0x7f) as u8,
        ]);
        tag.extend_from_slice(&body);
        tag
    }

    #[test]
    fn repairs_wrongly_labelled_chinese_without_changing_ascii_title() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let artwork = vec![0_u8; 3 * 1024 * 1024];
        std::fs::write(
            file.path(),
            fixture(&[
                (b"APIC", 0, &artwork),
                (b"TIT2", 0, b"I Believe"),
                (b"TALB", 0, &hex_bytes()),
                (b"TPE1", 0, &[0xcb, 0xef, 0xe9, 0xaa]),
            ]),
        )
        .unwrap();
        let fixed = corrected_text(file.path()).unwrap();
        assert_eq!(fixed.title.as_deref(), Some("I Believe"));
        assert_eq!(fixed.album.as_deref(), Some("缘份的天空"));
        assert_eq!(fixed.artist.as_deref(), Some("孙楠"));
    }

    fn hex_bytes() -> [u8; 10] {
        [0xd4, 0xb5, 0xb7, 0xdd, 0xb5, 0xc4, 0xcc, 0xec, 0xbf, 0xd5]
    }

    #[test]
    fn leaves_valid_western_latin1_and_unicode_frames_to_lofty() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            fixture(&[(b"TPE1", 0, b"Bj\xf6rk"), (b"TALB", 0, b"J\xf3ga")]),
        )
        .unwrap();
        assert!(corrected_text(file.path()).is_none());
        std::fs::write(file.path(), fixture(&[(b"TPE1", 1, b"\xff\xfeB\0j\0")])).unwrap();
        assert!(corrected_text(file.path()).is_none());
    }

    #[test]
    fn only_suspicious_display_text_triggers_a_raw_tag_recheck() {
        assert!(looks_misdecoded(&["Ôµ·ÝµÄÌì¿Õ", "Ëïéª"]));
        assert!(!looks_misdecoded(&["Björk", "Jóga"]));
        assert!(!looks_misdecoded(&["孙楠", "缘份的天空"]));
    }
}
