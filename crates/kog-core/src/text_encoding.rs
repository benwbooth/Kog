//! Shared decoding for metadata formats that do not declare a character set.

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{UTF_16BE, UTF_16LE};

/// Decode user-visible metadata, respecting Unicode BOMs and using
/// chardetng's language-aware legacy-encoding detector as a fallback.
pub fn decode(bytes: &[u8]) -> String {
    if let Some(bytes) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    // Test UTF-32 before UTF-16 because the little-endian UTF-32 BOM starts
    // with the complete little-endian UTF-16 BOM.
    if let Some(bytes) = bytes.strip_prefix(&[0xff, 0xfe, 0x00, 0x00]) {
        return decode_utf32(bytes, u32::from_le_bytes).unwrap_or_default();
    }
    if let Some(bytes) = bytes.strip_prefix(&[0x00, 0x00, 0xfe, 0xff]) {
        return decode_utf32(bytes, u32::from_be_bytes).unwrap_or_default();
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xff, 0xfe]) {
        return UTF_16LE.decode_without_bom_handling(bytes).0.into_owned();
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xfe, 0xff]) {
        return UTF_16BE.decode_without_bom_handling(bytes).0.into_owned();
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }
    if let Some(text) = decode_bomless_japanese_utf16(bytes) {
        return text;
    }

    let mut detector = EncodingDetector::new(Iso2022JpDetection::Allow);
    detector.feed(bytes, true);
    detector
        .guess(None, Utf8Detection::Allow)
        .decode_without_bom_handling(bytes)
        .0
        .into_owned()
}

fn decode_utf32(bytes: &[u8], order: fn([u8; 4]) -> u32) -> Option<String> {
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    bytes
        .chunks_exact(4)
        .map(|chunk| char::from_u32(order(chunk.try_into().expect("four-byte UTF-32 unit"))))
        .collect()
}

fn decode_bomless_japanese_utf16(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 4 || !bytes.len().is_multiple_of(2) {
        return None;
    }
    let little = decode_utf16(bytes, u16::from_le_bytes)?;
    let big = decode_utf16(bytes, u16::from_be_bytes)?;
    let little_score = japanese_text_score(&little);
    let big_score = japanese_text_score(&big);
    let little_kana = japanese_kana_count(&little);
    let big_kana = japanese_kana_count(&big);
    let (text, score, other_score, kana) = if little_score >= big_score {
        (little, little_score, big_score, little_kana)
    } else {
        (big, big_score, little_score, big_kana)
    };
    let nul_bytes = bytes.iter().filter(|byte| **byte == 0).count();
    let has_utf16_shape = nul_bytes * 4 >= bytes.len();
    // A decisive score keeps arbitrary single-byte metadata from being
    // mistaken for BOM-less UTF-16 merely because pairs happen to form CJK.
    (score >= 4 && score >= other_score.saturating_add(3) && (kana >= 2 || has_utf16_shape))
        .then_some(text)
}

fn decode_utf16(bytes: &[u8], order: fn([u8; 2]) -> u16) -> Option<String> {
    String::from_utf16(
        &bytes
            .chunks_exact(2)
            .map(|chunk| order(chunk.try_into().expect("two-byte UTF-16 unit")))
            .collect::<Vec<_>>(),
    )
    .ok()
}

fn japanese_text_score(text: &str) -> usize {
    text.chars()
        .map(|character| match character {
            '\u{3040}'..='\u{30ff}' => 3, // Hiragana and Katakana
            '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' => 2,
            '\u{3000}'..='\u{303f}' | '\u{ff00}'..='\u{ffef}' => 1,
            _ => 0,
        })
        .sum()
}

fn japanese_kana_count(text: &str) -> usize {
    text.chars()
        .filter(|character| matches!(character, '\u{3040}'..='\u{30ff}'))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::{SHIFT_JIS, WINDOWS_1251, WINDOWS_1252};

    #[test]
    fn preserves_utf8_and_decodes_unicode_boms() {
        assert_eq!(decode("Beyoncé".as_bytes()), "Beyoncé");
        assert_eq!(decode(&[0xff, 0xfe, b'K', 0, b'o', 0, b'g', 0]), "Kog");
        assert_eq!(decode(&[0xfe, 0xff, 0, b'K', 0, b'o', 0, b'g']), "Kog");
        assert_eq!(
            decode(&[0xff, 0xfe, 0, 0, b'K', 0, 0, 0, b'o', 0, 0, 0]),
            "Ko"
        );
        assert_eq!(
            decode(&[0, 0, 0xfe, 0xff, 0, 0, 0, b'K', 0, 0, 0, b'o']),
            "Ko"
        );
    }

    #[test]
    fn detects_common_legacy_music_metadata_encodings() {
        for (encoding, expected) in [
            (WINDOWS_1252, "Björk – Jóga"),
            (WINDOWS_1251, "Кино – Звезда"),
            (SHIFT_JIS, "クロノ・トリガー"),
        ] {
            let (bytes, _, errors) = encoding.encode(expected);
            assert!(!errors);
            assert_eq!(decode(&bytes), expected);
        }
    }

    #[test]
    fn recognizes_bomless_japanese_utf16_without_guessing_western_bytes() {
        let japanese = "クロノ・トリガー";
        let utf16 = japanese.encode_utf16().collect::<Vec<_>>();
        let little = utf16
            .iter()
            .flat_map(|unit| unit.to_le_bytes())
            .collect::<Vec<_>>();
        let big = utf16
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect::<Vec<_>>();
        assert_eq!(decode(&little), japanese);
        assert_eq!(decode(&big), japanese);

        let (western, _, errors) = WINDOWS_1252.encode("Björk – Jóga");
        assert!(!errors);
        assert_eq!(decode(&western), "Björk – Jóga");
    }
}
