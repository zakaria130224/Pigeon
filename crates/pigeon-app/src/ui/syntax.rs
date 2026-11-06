//! Colouring text: variables, JSON, JavaScript, Python and shell.

use super::palette;
use eframe::egui::{self, Align, Color32, FontId, Layout, Sense, Stroke, TextFormat, Ui, Vec2, text::LayoutJob};
use pigeon_core::http::Vars;

/// Text field that highlights `{{variables}}` (green when known, red when missing).
pub fn var_layouter(
    vars: &Vars,
    dark: bool,
    mono: bool,
) -> impl FnMut(&Ui, &dyn egui::TextBuffer, f32) -> std::sync::Arc<egui::Galley> + '_ {
    move |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
        let p = palette(dark);
        let text = buf.as_str();
        let font = if mono { FontId::monospace(12.5) } else { FontId::proportional(13.0) };
        let mut job = LayoutJob::default();
        job.wrap.max_width = wrap;
        let mut rest = text;
        while let Some(s) = rest.find("{{") {
            job.append(&rest[..s], 0.0, TextFormat::simple(font.clone(), p.text));
            let after = &rest[s + 2..];
            if let Some(e) = after.find("}}") {
                let name = after[..e].trim();
                let known = vars.has(name);
                let col = if known { p.ok } else { p.err };
                let mut fmt = TextFormat::simple(font.clone(), col);
                fmt.background =
                    if dark { Color32::from_rgba_unmultiplied(255, 255, 255, 14) } else { Color32::from_rgba_unmultiplied(0, 0, 0, 10) };
                job.append(&rest[s..s + 2 + e + 2], 0.0, fmt);
                rest = &after[e + 2..];
            } else {
                job.append(&rest[s..], 0.0, TextFormat::simple(font.clone(), p.text));
                rest = "";
            }
        }
        job.append(rest, 0.0, TextFormat::simple(font, p.text));
        ui.fonts_mut(|f| f.layout_job(job))
    }
}

/// Simple JSON syntax colouring.
pub fn json_job(text: &str, dark: bool, wrap: f32) -> LayoutJob {
    let p = palette(dark);
    let font = FontId::monospace(12.5);
    let (key, string, number, kw, punct) = if dark {
        (
            Color32::from_rgb(0x9c, 0xdc, 0xfe),
            Color32::from_rgb(0xce, 0x91, 0x78),
            Color32::from_rgb(0xb5, 0xce, 0xa8),
            Color32::from_rgb(0x56, 0x9c, 0xd6),
            p.weak,
        )
    } else {
        (
            Color32::from_rgb(0xa3, 0x15, 0x15),
            Color32::from_rgb(0x0f, 0x5a, 0xa8),
            Color32::from_rgb(0x09, 0x86, 0x58),
            Color32::from_rgb(0x00, 0x00, 0xff),
            Color32::from_rgb(0x55, 0x55, 0x55),
        )
    };
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let bytes = text.as_bytes();
    let mut i = 0;
    let push = |job: &mut LayoutJob, s: &str, c: Color32| job.append(s, 0.0, TextFormat::simple(font.clone(), c));
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(bytes.len());
            // key if next non-space is ':'
            let mut j = i;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let is_key = j < bytes.len() && bytes[j] == b':';
            push(&mut job, &text[start..i], if is_key { key } else { string });
        } else if c == b'-' || c.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || matches!(bytes[i], b'.' | b'e' | b'E' | b'+' | b'-')) {
                i += 1;
            }
            push(&mut job, &text[start..i], number);
        } else if c.is_ascii_alphabetic() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            push(&mut job, &text[start..i], kw);
        } else {
            let start = i;
            while i < bytes.len() && !(bytes[i] == b'"' || bytes[i] == b'-' || bytes[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            if i == start {
                // non-ascii char: advance to next char boundary
                i += 1;
                while i < bytes.len() && !text.is_char_boundary(i) {
                    i += 1;
                }
            }
            push(&mut job, &text[start..i], punct);
        }
    }
    job
}

/// Turns code into coloured text: the code, whether the theme is dark, and the wrap width.
pub type CodeColouring = fn(&str, bool, f32) -> LayoutJob;

/// Colours shared by the code languages.
struct CodeColours {
    keyword: Color32,
    string: Color32,
    number: Color32,
    comment: Color32,
}

fn code_colours(dark: bool) -> CodeColours {
    if dark {
        CodeColours {
            keyword: Color32::from_rgb(0x56, 0x9c, 0xd6),
            string: Color32::from_rgb(0xce, 0x91, 0x78),
            number: Color32::from_rgb(0xb5, 0xce, 0xa8),
            comment: Color32::from_rgb(0x6a, 0x99, 0x55),
        }
    } else {
        CodeColours {
            keyword: Color32::from_rgb(0x00, 0x00, 0xc8),
            string: Color32::from_rgb(0xa3, 0x15, 0x15),
            number: Color32::from_rgb(0x09, 0x86, 0x58),
            comment: Color32::from_rgb(0x3f, 0x7f, 0x3f),
        }
    }
}

/// What differs between the languages `script_job` colours.
struct Language {
    line_comment: &'static str,
    block_comments: bool,
    keywords: &'static [&'static str],
}

const JAVASCRIPT: Language = Language {
    line_comment: "//",
    block_comments: true,
    keywords: &[
        "async",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "default",
        "delete",
        "else",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "in",
        "let",
        "new",
        "null",
        "of",
        "return",
        "switch",
        "throw",
        "true",
        "try",
        "typeof",
        "undefined",
        "var",
        "while",
        "this",
    ],
};

const PYTHON: Language = Language {
    line_comment: "#",
    block_comments: false,
    keywords: &[
        "False", "None", "True", "and", "as", "async", "await", "break", "class", "continue", "def", "elif", "else", "except", "finally",
        "for", "from", "if", "import", "in", "is", "lambda", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield",
    ],
};

/// Simple JavaScript syntax colouring.
pub fn js_job(text: &str, dark: bool, wrap: f32) -> LayoutJob {
    script_job(text, dark, wrap, &JAVASCRIPT)
}

/// Simple Python syntax colouring.
pub fn python_job(text: &str, dark: bool, wrap: f32) -> LayoutJob {
    script_job(text, dark, wrap, &PYTHON)
}

fn script_job(text: &str, dark: bool, wrap: f32, language: &Language) -> LayoutJob {
    let p = palette(dark);
    let font = FontId::monospace(12.5);
    let CodeColours { keyword, string, number, comment } = code_colours(dark);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let push = |job: &mut LayoutJob, s: &str, c: Color32| job.append(s, 0.0, TextFormat::simple(font.clone(), c));
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let byte_at = |i: usize| chars.get(i).map(|c| c.0).unwrap_or(text.len());
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let c = chars[i].1;
        let next = chars.get(i + 1).map(|c| c.1);
        let color = if text[byte_at(i)..].starts_with(language.line_comment) {
            while i < chars.len() && chars[i].1 != '\n' {
                i += 1;
            }
            comment
        } else if language.block_comments && c == '/' && next == Some('*') {
            i += 2;
            while i < chars.len() && !(chars[i].1 == '*' && chars.get(i + 1).map(|c| c.1) == Some('/')) {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            comment
        } else if matches!(c, '"' | '\'' | '`') {
            i += 1;
            while i < chars.len() && chars[i].1 != c {
                if chars[i].1 == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(chars.len());
            string
        } else if c.is_ascii_digit() {
            while i < chars.len() && (chars[i].1.is_ascii_alphanumeric() || chars[i].1 == '.') {
                i += 1;
            }
            number
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            while i < chars.len() && (chars[i].1.is_alphanumeric() || chars[i].1 == '_' || chars[i].1 == '$') {
                i += 1;
            }
            let word = &text[byte_at(start)..byte_at(i)];
            if language.keywords.contains(&word) { keyword } else { p.text }
        } else {
            i += 1;
            p.text
        };
        push(&mut job, &text[byte_at(start)..byte_at(i)], color);
    }
    job
}

/// Simple shell colouring: the command and its flags, and quoted text.
pub fn shell_job(text: &str, dark: bool, wrap: f32) -> LayoutJob {
    let p = palette(dark);
    let font = FontId::monospace(12.5);
    let CodeColours { keyword, string, .. } = code_colours(dark);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let mut command = true;
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let (len, color) = if c == '\'' {
            // Nothing can be escaped inside single quotes.
            (rest[1..].find('\'').map_or(rest.len(), |end| end + 2), string)
        } else if c == '"' {
            (double_quoted_len(rest), string)
        } else if c == '\\' {
            // `\'` is how a single quote is written between two quoted parts, so it belongs to the string.
            let escaped = rest[1..].chars().next().map_or(0, char::len_utf8);
            (1 + escaped, if rest[1..].starts_with('\'') { string } else { p.weak })
        } else if c.is_whitespace() {
            (rest.find(|c: char| !c.is_whitespace()).unwrap_or(rest.len()), p.text)
        } else {
            let len = rest.find(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '\\')).unwrap_or(rest.len());
            let is_keyword = command || c == '-';
            command = false;
            (len, if is_keyword { keyword } else { p.text })
        };
        job.append(&rest[..len], 0.0, TextFormat::simple(font.clone(), color));
        rest = &rest[len..];
    }
    job
}

/// Length of the `"..."` that `text` starts with, including both quotes.
fn double_quoted_len(text: &str) -> usize {
    let mut escaped = false;
    for (i, c) in text.char_indices().skip(1) {
        match c {
            '"' if !escaped => return i + 1,
            '\\' if !escaped => escaped = true,
            _ => escaped = false,
        }
    }
    text.len()
}

/// Read-only code with line numbers, which stay in place when the code scrolls sideways. Long lines scroll rather than wrap.
pub fn code_view(ui: &mut Ui, id_salt: &str, code: &str, dark: bool, colours: CodeColouring) {
    // Past this size colouring every frame would slow the window down, the same limit as the response body.
    const MAX_COLOURED: usize = 2_000_000;
    let p = palette(dark);
    let font = FontId::monospace(12.5);
    let lines = code.split('\n').count();
    let digits = lines.to_string().len().max(2);
    let numbers = (1..=lines).map(|n| format!("{n:>digits$}")).collect::<Vec<_>>().join("\n");
    egui::Frame::new().fill(p.header).stroke(Stroke::new(1.0, p.border)).corner_radius(4).inner_margin(8).show(ui, |ui| {
        ui.allocate_ui_with_layout(ui.available_size(), Layout::left_to_right(Align::Min), |ui| {
            let gutter = ui.fonts_mut(|f| f.layout_no_wrap(numbers, font.clone(), p.weak));
            let (gutter_rect, _) = ui.allocate_exact_size(Vec2::new(gutter.size().x, ui.available_height()), Sense::hover());
            ui.add_space(4.0);
            let scroll = egui::ScrollArea::both().id_salt(id_salt).auto_shrink([false, false]).show(ui, |ui| {
                let mut lay = |ui: &Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
                    let text = buf.as_str();
                    let job = if text.len() < MAX_COLOURED {
                        colours(text, dark, f32::INFINITY)
                    } else {
                        LayoutJob::simple(text.to_string(), font.clone(), p.text, f32::INFINITY)
                    };
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                let mut text = code;
                ui.add(
                    egui::TextEdit::multiline(&mut text)
                        .frame(egui::Frame::NONE)
                        .desired_rows(1)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut lay),
                );
            });
            let top = gutter_rect.min - Vec2::new(0.0, scroll.state.unclamped_offset().y);
            ui.painter_at(gutter_rect).galley(top, gutter, p.weak);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pieces of a job as (text, colour).
    fn pieces(job: &LayoutJob) -> Vec<(&str, Color32)> {
        job.sections.iter().map(|s| (&job.text[s.byte_range.start.0..s.byte_range.end.0], s.format.color)).collect()
    }

    /// Neighbouring text of the same colour shares a piece, so this looks for the piece holding `text`.
    fn colour_of(job: &LayoutJob, text: &str) -> Color32 {
        pieces(job).iter().find(|(t, _)| t.contains(text)).unwrap_or_else(|| panic!("no {text:?} in {:?}", pieces(job))).1
    }

    #[test]
    fn shell_colours_the_command_flags_and_quotes() {
        let c = code_colours(true);
        let code = "curl --location --request GET \\\n  'https://example.com/a' \\\n  --header \"A: b\"";
        let job = shell_job(code, true, f32::INFINITY);
        assert_eq!(job.text, code);
        assert_eq!(colour_of(&job, "curl"), c.keyword);
        assert_eq!(colour_of(&job, "--header"), c.keyword);
        assert_eq!(colour_of(&job, "GET"), palette(true).text);
        assert_eq!(colour_of(&job, "'https://example.com/a'"), c.string);
        assert_eq!(colour_of(&job, "\"A: b\""), c.string);
    }

    #[test]
    fn shell_keeps_escaped_quotes_inside_the_string() {
        let c = code_colours(true);
        let job = shell_job(r"curl 'it'\''s --not-a-flag é'", true, f32::INFINITY);
        assert_eq!(job.text, r"curl 'it'\''s --not-a-flag é'");
        assert!(pieces(&job).iter().skip(2).all(|(_, colour)| *colour == c.string));
    }

    #[test]
    fn shell_survives_unfinished_text() {
        for code in ["curl 'open", "curl \"open\\", "curl \\", "'", "\""] {
            assert_eq!(shell_job(code, true, f32::INFINITY).text, code);
        }
    }

    #[test]
    fn python_has_its_own_keywords_and_comments() {
        let c = code_colours(true);
        let job = python_job("import requests\n# note\nx = None // 2\nconst = \"a\"", true, f32::INFINITY);
        assert_eq!(colour_of(&job, "import"), c.keyword);
        assert_eq!(colour_of(&job, "None"), c.keyword);
        assert_eq!(colour_of(&job, "# note"), c.comment);
        assert_eq!(colour_of(&job, "const"), palette(true).text);
        assert_eq!(colour_of(&job, "\"a\""), c.string);
    }

    #[test]
    fn javascript_colours_stay_the_same() {
        let c = code_colours(true);
        let job = js_job("const a = 1; // b\n/* c */ typeof # d", true, f32::INFINITY);
        assert_eq!(colour_of(&job, "const"), c.keyword);
        assert_eq!(colour_of(&job, "typeof"), c.keyword);
        assert_eq!(colour_of(&job, "// b"), c.comment);
        assert_eq!(colour_of(&job, "/* c */"), c.comment);
    }
}
