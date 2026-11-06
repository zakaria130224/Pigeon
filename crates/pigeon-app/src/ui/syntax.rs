//! Colouring text: variables, JSON and JavaScript.

use super::palette;
use eframe::egui::{self, Color32, FontId, TextFormat, Ui, text::LayoutJob};
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

/// Simple JavaScript syntax colouring.
pub fn js_job(text: &str, dark: bool, wrap: f32) -> LayoutJob {
    const KEYWORDS: [&str; 26] = [
        "async", "await", "break", "case", "catch", "class", "const", "continue", "default", "delete", "else", "false", "finally", "for",
        "function", "if", "in", "let", "new", "null", "of", "return", "switch", "throw", "true", "try",
    ];
    const MORE_KEYWORDS: [&str; 5] = ["typeof", "undefined", "var", "while", "this"];
    let p = palette(dark);
    let font = FontId::monospace(12.5);
    let (keyword, string, number, comment) = if dark {
        (
            Color32::from_rgb(0x56, 0x9c, 0xd6),
            Color32::from_rgb(0xce, 0x91, 0x78),
            Color32::from_rgb(0xb5, 0xce, 0xa8),
            Color32::from_rgb(0x6a, 0x99, 0x55),
        )
    } else {
        (
            Color32::from_rgb(0x00, 0x00, 0xc8),
            Color32::from_rgb(0xa3, 0x15, 0x15),
            Color32::from_rgb(0x09, 0x86, 0x58),
            Color32::from_rgb(0x3f, 0x7f, 0x3f),
        )
    };
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
        let color = if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i].1 != '\n' {
                i += 1;
            }
            comment
        } else if c == '/' && next == Some('*') {
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
            if KEYWORDS.contains(&word) || MORE_KEYWORDS.contains(&word) { keyword } else { p.text }
        } else {
            i += 1;
            p.text
        };
        push(&mut job, &text[byte_at(start)..byte_at(i)], color);
    }
    job
}
