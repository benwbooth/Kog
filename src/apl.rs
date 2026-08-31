//! Parser for Monkey's Audio Image Link (APL) files.

use std::path::{Path, PathBuf};

const HEADER: &[u8] = b"[Monkey's Audio Image Link File]";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AplLink {
    pub audio_path: PathBuf,
    pub start_frame: u64,
    pub end_frame: Option<u64>,
}

impl AplLink {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path)
            .map_err(|error| format!("opening APL link {}: {error}", path.display()))?;
        Self::parse(path, &bytes)
    }

    fn parse(path: &Path, bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < HEADER.len() || !bytes[..HEADER.len()].eq_ignore_ascii_case(HEADER) {
            return Err(format!(
                "{} is not a Monkey's Audio Image Link file",
                path.display()
            ));
        }
        let body = bytes[HEADER.len()..]
            .strip_prefix(b"\r\n")
            .or_else(|| bytes[HEADER.len()..].strip_prefix(b"\n"))
            .ok_or_else(|| format!("{} has an invalid APL header line ending", path.display()))?;

        let mut image_file = None;
        let mut start_frame = 0_u64;
        let mut finish_frame = 0_u64;
        for raw_line in body.split(|byte| *byte == b'\n') {
            let raw_line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
            if raw_line.first() == Some(&b'-') {
                break;
            }
            let Ok(line) = std::str::from_utf8(raw_line) else {
                return Err(format!("{} contains non-UTF-8 APL fields", path.display()));
            };
            let Some((field, value)) = line.split_once('=') else {
                continue;
            };
            let field = field.trim();
            let value = value.trim();
            if field.eq_ignore_ascii_case("Image File") {
                image_file = Some(value.to_owned());
            } else if field.eq_ignore_ascii_case("Start Block") {
                start_frame = parse_frame(path, "Start Block", value)?;
            } else if field.eq_ignore_ascii_case("Finish Block") {
                finish_frame = parse_frame(path, "Finish Block", value)?;
            }
        }

        let image_file = image_file
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("{} has no Image File field", path.display()))?;
        if image_file.contains("://") {
            return Err(format!(
                "{} references a URL; Kog's APL backend currently accepts filesystem image paths",
                path.display()
            ));
        }
        let normalized = if Path::new(&image_file).is_absolute() {
            image_file
        } else {
            image_file.replace('\\', "/")
        };
        let audio_path = if Path::new(&normalized).is_absolute() {
            PathBuf::from(normalized)
        } else {
            path.parent()
                .unwrap_or_else(|| Path::new("."))
                .join(normalized)
        };
        let audio_path = audio_path.canonicalize().map_err(|error| {
            format!(
                "resolving image file referenced by {} ({}): {error}",
                path.display(),
                audio_path.display()
            )
        })?;
        if !audio_path.is_file() {
            return Err(format!(
                "image file referenced by {} is not a file: {}",
                path.display(),
                audio_path.display()
            ));
        }

        Ok(Self {
            audio_path,
            start_frame,
            end_frame: (finish_frame > start_frame).then_some(finish_frame),
        })
    }
}

fn parse_frame(path: &Path, field: &str, value: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|error| {
        format!(
            "{} has an invalid {field} value {value:?}: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> (Self, PathBuf, PathBuf) {
            let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "kog-apl-parser-fixture-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir_all(directory.join("audio")).expect("create fixture directory");
            let audio = directory.join("audio/image.wav");
            std::fs::write(&audio, b"fixture").expect("write referenced file");
            let apl = directory.join("track.apl");
            (Self(directory), apl, audio)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn parses_cog_header_relative_windows_path_and_frame_range() {
        let (_fixture, apl, audio) = Fixture::new();
        std::fs::write(
            &apl,
            concat!(
                "[monkey's audio image link file]\r\n",
                "Image File=audio\\image.wav\r\n",
                "Start Block=200\r\n",
                "Finish Block=600\r\n",
                "----- APE TAG (DO NOT TOUCH!!!) -----\r\n",
                "Title=Ignored by the link parser\0"
            ),
        )
        .expect("write APL fixture");

        assert_eq!(
            AplLink::open(&apl).expect("parse APL fixture"),
            AplLink {
                audio_path: audio.canonicalize().expect("canonical audio path"),
                start_frame: 200,
                end_frame: Some(600),
            }
        );
    }

    #[test]
    fn rejects_bad_headers_missing_files_and_remote_images() {
        let (_fixture, apl, _audio) = Fixture::new();
        std::fs::write(&apl, b"not an APL").expect("write invalid APL");
        assert!(
            AplLink::open(&apl)
                .expect_err("reject header")
                .contains("not a Monkey")
        );

        std::fs::write(
            &apl,
            b"[Monkey's Audio Image Link File]\r\nImage File=missing.ape\r\n",
        )
        .expect("write missing link");
        assert!(
            AplLink::open(&apl)
                .expect_err("reject missing image")
                .contains("resolving")
        );

        std::fs::write(
            &apl,
            b"[Monkey's Audio Image Link File]\r\nImage File=https://example.invalid/image.ape\r\n",
        )
        .expect("write remote link");
        assert!(
            AplLink::open(&apl)
                .expect_err("reject URL image")
                .contains("references a URL")
        );
    }

    #[test]
    fn accepts_portable_lf_line_endings() {
        let (_fixture, apl, audio) = Fixture::new();
        std::fs::write(
            &apl,
            b"[Monkey's Audio Image Link File]\nImage File=audio/image.wav\nStart Block=3\nFinish Block=9\n",
        )
        .expect("write LF APL fixture");

        let link = AplLink::open(&apl).expect("parse LF APL fixture");
        assert_eq!(link.audio_path, audio.canonicalize().unwrap());
        assert_eq!(link.start_frame, 3);
        assert_eq!(link.end_frame, Some(9));
    }
}
