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
    fn chapters_render_as_html() {
        for chapter in CHAPTERS {
            let page = html(chapter.markdown);
            assert!(page.starts_with("<h1>"), "{}", chapter.title);
            assert!(!page.contains("```") && !page.contains("**"), "{}", chapter.title);
            assert_eq!(page.matches("<table>").count(), page.matches("</table>").count());
            assert_eq!(page.matches("<pre>").count(), chapter.markdown.matches("```").count() / 2);
        }
        let sample = html("A `c4` and **bold** *it* < 3\n\n| a | b \\| c |\n| --- | --- |\n| 1 | `x\\|y` |\n");
        assert!(sample.contains("<code>c4</code>") && sample.contains("<strong>bold</strong>") && sample.contains("&lt; 3"), "{sample}");
        assert!(sample.contains("<th>b | c</th>"), "{sample}");
    }

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

/// Qt's rich text understands a small part of CSS: block margins, colours,
/// fonts and line height, but no padding or borders on blocks. Spacing that
/// matches the web guide comes from margins, and boxes from tables.
const QT_STYLE: &str = "
body { color: #dce3e8; font-size: 15px; line-height: 140%; }
h1 { color: #edf4f7; font-size: 28px; font-weight: 600; margin-top: 4px; margin-bottom: 18px; }
h2 { color: #edf4f7; font-size: 20px; font-weight: 600; margin-top: 30px; margin-bottom: 10px; }
h3 { color: #edf4f7; font-size: 17px; font-weight: 600; margin-top: 22px; margin-bottom: 8px; }
h4 { color: #edf4f7; font-size: 15px; font-weight: 600; margin-top: 18px; margin-bottom: 6px; }
p { margin-top: 0px; margin-bottom: 14px; }
ul, ol { margin-top: 0px; margin-bottom: 14px; }
li { margin-bottom: 6px; }
code { font-family: monospace; color: #c3e88d; background-color: #18262e; }
pre { font-family: monospace; font-size: 14px; line-height: 135%; margin: 0px; color: #dce3e8; }
pre code { color: #dce3e8; background-color: #0b1216; }
th { color: #edf4f7; font-weight: 600; text-align: left; line-height: 120%; }
td { vertical-align: top; line-height: 120%; }
";

/// A chapter as HTML for Qt's rich text, laid out like the web guide.
pub fn rich_text(markdown: &str) -> String {
    let body = html(markdown)
        // A trailing newline would leave an empty line at the box's foot.
        .replace("\n</code></pre>", "</code></pre>")
        // Code blocks get a padded dark box from a one-cell table.
        .replace(
            "<pre>",
            "<table width=\"100%\" cellspacing=\"0\" cellpadding=\"14\" bgcolor=\"#0b1216\" \
             style=\"margin-top: 4px; margin-bottom: 18px; border-color: #1f2f38; border-style: solid;\" border=\"1\"><tr><td><pre>",
        )
        .replace("</pre>", "</pre></td></tr></table>")
        .replace("<th>", "<th bgcolor=\"#152229\">")
        .replace(
            "<table>",
            "<table cellspacing=\"0\" cellpadding=\"7\" border=\"1\" \
             style=\"margin-top: 6px; margin-bottom: 18px; border-color: #253944; border-style: solid; border-collapse: collapse;\">",
        );
    format!("<html><head><style>{QT_STYLE}</style></head><body>{body}</body></html>")
}

/// Render a chapter's Markdown as HTML. Handles the subset the guide uses:
/// headings, paragraphs, lists, tables, fenced code, and inline code,
/// bold and italics.
pub fn html(markdown: &str) -> String {
    let mut out = String::new();
    let lines: Vec<&str> = markdown.lines().collect();
    let mut index = 0;
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |paragraph: &mut Vec<&str>, out: &mut String| {
        if !paragraph.is_empty() {
            out.push_str(&format!("<p>{}</p>\n", inline(&paragraph.join(" "))));
            paragraph.clear();
        }
    };
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            flush(&mut paragraph, &mut out);
            index += 1;
            let mut code = String::new();
            while index < lines.len() && !lines[index].trim_start().starts_with("```") {
                code.push_str(&escape(lines[index]));
                code.push('\n');
                index += 1;
            }
            out.push_str(&format!("<pre><code>{code}</code></pre>\n"));
        } else if let Some(level) = (1..=4).find(|n| trimmed.starts_with(&format!("{} ", "#".repeat(*n)))) {
            flush(&mut paragraph, &mut out);
            out.push_str(&format!("<h{level}>{}</h{level}>\n", inline(&trimmed[level + 1..])));
        } else if trimmed.starts_with('|') {
            flush(&mut paragraph, &mut out);
            let row = |line: &str| -> Vec<String> {
                let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
                // `\|` inside a cell is a literal bar.
                inner
                    .replace("\\|", "\u{0}")
                    .split('|')
                    .map(|cell| cell.trim().replace('\u{0}', "|"))
                    .collect()
            };
            out.push_str("<table>\n<thead><tr>");
            for cell in row(line) {
                out.push_str(&format!("<th>{}</th>", inline(&cell)));
            }
            out.push_str("</tr></thead>\n<tbody>\n");
            index += 2; // header and separator rows
            while index < lines.len() && lines[index].trim_start().starts_with('|') {
                out.push_str("<tr>");
                for cell in row(lines[index]) {
                    out.push_str(&format!("<td>{}</td>", inline(&cell)));
                }
                out.push_str("</tr>\n");
                index += 1;
            }
            out.push_str("</tbody></table>\n");
            continue;
        } else if trimmed.starts_with("- ") || ordered(trimmed).is_some() {
            flush(&mut paragraph, &mut out);
            let tag = if trimmed.starts_with("- ") { "ul" } else { "ol" };
            out.push_str(&format!("<{tag}>\n"));
            while index < lines.len() {
                let item = lines[index].trim_start();
                let text = if let Some(rest) = item.strip_prefix("- ") {
                    rest
                } else if let Some(rest) = ordered(item) {
                    rest
                } else {
                    break;
                };
                let mut text = text.to_owned();
                // Continuation lines are indented under their item.
                while index + 1 < lines.len()
                    && lines[index + 1].starts_with("  ")
                    && !lines[index + 1].trim_start().starts_with("- ")
                    && ordered(lines[index + 1].trim_start()).is_none()
                {
                    index += 1;
                    text.push(' ');
                    text.push_str(lines[index].trim());
                }
                out.push_str(&format!("<li>{}</li>\n", inline(&text)));
                index += 1;
            }
            out.push_str(&format!("</{tag}>\n"));
            continue;
        } else if trimmed.is_empty() {
            flush(&mut paragraph, &mut out);
        } else {
            paragraph.push(trimmed);
        }
        index += 1;
    }
    flush(&mut paragraph, &mut out);
    out
}

fn ordered(line: &str) -> Option<&str> {
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    (digits > 0).then(|| line[digits..].strip_prefix(". ")).flatten()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Inline code, bold and italics; everything else is escaped text.
fn inline(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(code) = rest.strip_prefix('`') {
            if let Some(end) = code.find('`') {
                out.push_str(&format!("<code>{}</code>", escape(&code[..end])));
                rest = &code[end + 1..];
                continue;
            }
        }
        if let Some(bold) = rest.strip_prefix("**") {
            if let Some(end) = bold.find("**") {
                out.push_str(&format!("<strong>{}</strong>", inline(&bold[..end])));
                rest = &bold[end + 2..];
                continue;
            }
        }
        if let Some(italic) = rest.strip_prefix('*') {
            if let Some(end) = italic.find('*') {
                out.push_str(&format!("<em>{}</em>", inline(&italic[..end])));
                rest = &italic[end + 1..];
                continue;
            }
        }
        let next = rest[1..].find(['`', '*']).map_or(rest.len(), |at| at + 1);
        out.push_str(&escape(&rest[..next]));
        rest = &rest[next..];
    }
    out
}
