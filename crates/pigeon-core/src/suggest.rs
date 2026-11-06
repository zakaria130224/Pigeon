//! Working out which variables to suggest while typing, and what the choice does to the text.

use crate::http::{VarHint, VarKind, Vars};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// `{{name}}` in URLs, headers, bodies and the like.
    Template,
    /// Also the name in `pm.environment.get("name")` and similar.
    Script,
}

/// A variable name being typed at the text cursor. Positions count characters, not bytes.
#[derive(Debug, PartialEq)]
pub struct Partial {
    start: usize,
    end: usize,
    pub query: String,
    pub closing: String,
    /// The closing text is already there, after `end`.
    pub closed: usize,
    only: Option<VarKind>,
}

fn name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '$')
}

/// The end of the name around the cursor, when `closer` already follows it.
fn existing_end(chars: &[char], cursor: usize, closer: &[char]) -> Option<usize> {
    let mut end = cursor;
    while end < chars.len() && name_char(chars[end]) {
        end += 1;
    }
    chars[end..].starts_with(closer).then_some(end)
}

fn find_template(chars: &[char], cursor: usize) -> Option<Partial> {
    let mut start = cursor;
    while start > 0 && (name_char(chars[start - 1]) || chars[start - 1] == ' ') {
        start -= 1;
    }
    if start < 2 || chars[start - 1] != '{' || chars[start - 2] != '{' || (start > 2 && chars[start - 3] == '{') {
        return None;
    }
    let query: String = chars[start..cursor].iter().collect::<String>().trim().to_string();
    Some(match existing_end(chars, cursor, &['}', '}']) {
        Some(end) => Partial { start, end, query, closing: String::new(), closed: 2, only: None },
        None => Partial { start, end: cursor, query, closing: "}}".into(), closed: 0, only: None },
    })
}

fn find_script(chars: &[char], cursor: usize) -> Option<Partial> {
    let mut start = cursor;
    while start > 0 && name_char(chars[start - 1]) {
        start -= 1;
    }
    let quote = *chars.get(start.checked_sub(1)?)?;
    if !matches!(quote, '"' | '\'' | '`') {
        return None;
    }
    let before: String = chars[start.saturating_sub(48)..start - 1].iter().collect();
    let call = before.trim_end().strip_suffix('(')?.trim_end();
    let (target, method) = call.rsplit_once('.')?;
    if !matches!(method, "get" | "set" | "has" | "unset") {
        return None;
    }
    let only = if target.ends_with("pm.environment") {
        Some(VarKind::Environment)
    } else if target.ends_with("pm.collectionVariables") {
        Some(VarKind::Collection)
    } else if target.ends_with("pm.globals") {
        Some(VarKind::Global)
    } else if target.ends_with("pm.variables") {
        None
    } else {
        return None;
    };
    let query: String = chars[start..cursor].iter().collect();
    Some(match existing_end(chars, cursor, &[quote]) {
        Some(end) => Partial { start, end, query, closing: String::new(), closed: 1, only },
        None => Partial { start, end: cursor, query, closing: quote.to_string(), closed: 0, only },
    })
}

pub fn find_partial(text: &str, cursor: usize, trigger: Trigger) -> Option<Partial> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let script = if trigger == Trigger::Script { find_script(&chars, cursor) } else { None };
    script.or_else(|| find_template(&chars, cursor))
}

/// How well a name fits what was typed: the start beats the middle, which beats scattered letters.
fn rank(name: &str, query: &str) -> Option<u8> {
    let name = name.to_lowercase();
    let query = query.to_lowercase();
    if name.starts_with(&query) {
        return Some(0);
    }
    if name.contains(&query) {
        return Some(1);
    }
    let mut letters = name.chars();
    query.chars().all(|q| letters.any(|n| n == q)).then_some(2)
}

pub fn candidates<'a>(vars: &'a Vars, partial: &Partial) -> Vec<&'a VarHint> {
    let mut found: Vec<(u8, &VarHint)> = vars
        .hints
        .iter()
        .filter(|h| match partial.only {
            Some(kind) => h.kind == kind,
            // Names like $guid only mean something inside {{ }}, and are noise until asked for.
            None => h.kind != VarKind::Dynamic || partial.closing == "}}" || partial.closed == 2 || partial.query.starts_with('$'),
        })
        .filter_map(|h| rank(&h.name, &partial.query).map(|r| (r, h)))
        .collect();
    found.sort_by_key(|(r, h)| (*r, h.kind == VarKind::Dynamic));
    found.into_iter().map(|(_, h)| h).collect()
}

/// Puts the chosen name in the text and returns where the text cursor belongs.
pub fn insert(text: &mut String, partial: &Partial, name: &str) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut out: String = chars[..partial.start].iter().collect();
    out.push_str(name);
    out.push_str(&partial.closing);
    out.extend(&chars[partial.end..]);
    *text = out;
    partial.start + name.chars().count() + partial.closing.chars().count() + partial.closed
}

/// The `{{variable}}` at a character position, if any.
pub fn var_at(text: &str, index: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i + 1 < chars.len() {
        if chars[i] == '{' && chars[i + 1] == '{' {
            let close = (i + 2..chars.len().saturating_sub(1)).find(|&j| chars[j] == '}' && chars[j + 1] == '}')?;
            if (i..=close + 2).contains(&index) {
                return Some(chars[i + 2..close].iter().collect::<String>().trim().to_string());
            }
            i = close + 2;
        } else {
            i += 1;
        }
    }
    None
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            let next = (diagonal + cost).min(row[j] + 1).min(row[j + 1] + 1);
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

/// The defined variable a mistyped name was probably meant to be.
pub fn closest<'a>(name: &str, vars: &'a Vars) -> Option<&'a str> {
    let wanted = name.to_lowercase();
    // Two swapped letters count as two edits, and that's the most common typo.
    let length = wanted.chars().count();
    let limit = if length <= 3 { 1 } else { (length / 3).max(2) };
    vars.hints
        .iter()
        .filter(|h| h.kind != VarKind::Dynamic)
        .map(|h| (distance(&wanted, &h.name.to_lowercase()), h.name.as_str()))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, n)| n)
}
