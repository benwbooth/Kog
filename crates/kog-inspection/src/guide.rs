//! The Kog MML guide: a book of chapters shipped with every frontend.
//! The chapters live in `docs/mml-guide` as Markdown.

/// One chapter: its title and its Markdown text (which starts with the title
/// as a level-one heading).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Chapter {
    pub title: &'static str,
    pub markdown: &'static str,
}

macro_rules! chapters {
    ($($file:literal),* $(,)?) => {
        [$(Chapter {
            title: title(include_str!(concat!("../../../docs/mml-guide/", $file))),
            markdown: include_str!(concat!("../../../docs/mml-guide/", $file)),
        }),*]
    };
}

const fn title(markdown: &'static str) -> &'static str {
    // The first line is "# N. Title"; keep the text after "# ".
    let bytes = markdown.as_bytes();
    let mut end = 0;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    let (_, rest) = bytes.split_at(2);
    let (line, _) = rest.split_at(end - 2);
    match std::str::from_utf8(line) {
        Ok(text) => text,
        Err(_) => "",
    }
}

pub const CHAPTERS: [Chapter; 17] = chapters![
    "01-introduction.md",
    "02-the-mml-view.md",
    "03-file-structure.md",
    "04-headers.md",
    "05-tracks-and-voices.md",
    "06-notes-and-pitch.md",
    "07-lengths-and-ties.md",
    "08-rests-hits-legato.md",
    "09-commands-and-parameters.md",
    "10-macros.md",
    "11-pitch-and-tuning-tables.md",
    "12-relative-sample-pitch.md",
    "13-how-scores-are-recorded.md",
    "14-formats.md",
    "15-grammar.md",
    "16-worked-example.md",
    "17-questions.md",
];

/// The whole guide as JSON, for frontends that cannot link Rust directly.
pub fn json() -> String {
    serde_json::to_string(&CHAPTERS).unwrap_or_else(|_| "[]".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mml::parse;

    #[test]
    fn chapters_have_titles_and_text() {
        for (index, chapter) in CHAPTERS.iter().enumerate() {
            assert!(chapter.title.starts_with(&format!("{}. ", index + 1)), "{}", chapter.title);
            assert!(chapter.markdown.len() > 500, "{} is too short", chapter.title);
        }
    }

    /// Every complete MML example in the guide must parse; the guide may not
    /// drift from the language.
    #[test]
    fn complete_examples_parse() {
        let mut checked = 0;
        for chapter in CHAPTERS {
            for block in chapter.markdown.split("```").skip(1).step_by(2) {
                if block.trim_start().starts_with("#KOG-MML") && !block.contains('…') {
                    let score = parse(block).unwrap_or_else(|error| panic!("{}: {error}\n{block}", chapter.title));
                    // Reading it back as Kog writes it gives the same score.
                    let again = parse(&crate::mml::encode(&score).text).unwrap();
                    assert_eq!(again, score, "{}", chapter.title);
                    checked += 1;
                }
            }
        }
        assert!(checked >= 1, "the grammar chapter has a complete example");
    }
}
