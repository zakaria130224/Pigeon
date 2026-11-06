//! The text form of a key and value table: one `key:value` per line, with `//` in front of a row that's off.

use super::Kv;

fn line_of(row: &Kv) -> String {
    format!("{}{}:{}", if row.enabled { "" } else { "//" }, row.key, row.value)
}

/// Rows as text. Rows with neither a key nor a value have no line.
pub fn kv_to_text(rows: &[Kv]) -> String {
    rows.iter().filter(|r| !(r.key.is_empty() && r.value.is_empty())).map(line_of).collect::<Vec<_>>().join("\n")
}

/// Rows from text. The text has no room for descriptions, so they come from the `known` row with the same key.
/// A line that still matches a `known` row exactly gives back that row, even if its key contains a colon.
pub fn kv_from_text(text: &str, known: &[Kv]) -> Vec<Kv> {
    let mut unused: Vec<&Kv> = known.iter().collect();
    let mut rows = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(i) = unused.iter().position(|r| line_of(r).trim() == line) {
            rows.push(unused.remove(i).clone());
            continue;
        }
        let (enabled, rest) = match line.strip_prefix("//") {
            Some(rest) => (false, rest),
            None => (true, line),
        };
        let (key, value) = rest.split_once(':').unwrap_or((rest, ""));
        let (key, value) = (key.trim(), value.trim());
        if key.is_empty() && value.is_empty() {
            continue;
        }
        let description = match unused.iter().position(|r| r.key == key) {
            Some(i) => unused.remove(i).description.clone(),
            None => String::new(),
        };
        rows.push(Kv { key: key.into(), value: value.into(), enabled, description });
    }
    rows
}

/// A table being edited as text. Every edit is applied straight away, so sending or saving never misses one.
#[derive(Clone, Debug, Default)]
pub struct BulkEdit {
    pub text: String,
    /// The rows when editing began, so a description survives its line being half typed.
    known: Vec<Kv>,
    /// The rows this text stands for.
    shown: Vec<Kv>,
}

impl BulkEdit {
    pub fn new(rows: &[Kv]) -> Self {
        BulkEdit { text: kv_to_text(rows), known: rows.to_vec(), shown: rows.to_vec() }
    }

    /// Start over if the rows were changed from elsewhere, such as by typing in the URL.
    pub fn refresh(&mut self, rows: &[Kv]) {
        if rows != self.shown {
            *self = BulkEdit::new(rows);
        }
    }

    /// Replace the rows with what the text says.
    pub fn apply(&mut self, rows: &mut Vec<Kv>) {
        *rows = kv_from_text(&self.text, &self.known);
        self.shown = rows.clone();
    }
}
