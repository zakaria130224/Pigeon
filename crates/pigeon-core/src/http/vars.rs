//! `{{variables}}` and the values behind them.

use crate::model::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    Environment,
    Collection,
    Global,
    Dynamic,
}

/// A variable as the suggestion list shows it.
#[derive(Clone, Debug)]
pub struct VarHint {
    pub name: String,
    pub value: String,
    pub kind: VarKind,
    /// Where it comes from, in plain words.
    pub place: String,
}

/// Names filled in fresh for each send.
pub const DYNAMIC_VARS: [(&str, &str); 6] = [
    ("$guid", "A new unique ID"),
    ("$randomUUID", "A new unique ID"),
    ("$timestamp", "The current time, in seconds"),
    ("$isoTimestamp", "The current date and time"),
    ("$randomInt", "A random number from 0 to 1000"),
    ("$randomBoolean", "true or false at random"),
];

/// Variable scopes, highest priority first.
#[derive(Default, Clone)]
pub struct Vars {
    pub map: HashMap<String, String>,
    pub hints: Vec<VarHint>,
}

impl Vars {
    pub fn build(env: Option<&Environment>, collection: Option<&Collection>, globals: &[Kv]) -> Self {
        let mut map = HashMap::new();
        for kv in globals.iter().filter(|k| k.enabled && !k.key.is_empty()) {
            map.insert(kv.key.clone(), kv.value.clone());
        }
        if let Some(c) = collection {
            for kv in c.variables.iter().filter(|k| k.enabled && !k.key.is_empty()) {
                map.insert(kv.key.clone(), kv.value.clone());
            }
        }
        if let Some(e) = env {
            for kv in e.values.iter().filter(|k| k.enabled && !k.key.is_empty()) {
                map.insert(kv.key.clone(), kv.value.clone());
            }
        }
        Vars { map, ..Default::default() }
    }

    pub fn has(&self, name: &str) -> bool {
        name.starts_with('$') || self.map.contains_key(name)
    }

    /// Replace `{{name}}` occurrences. Unknown names are left as-is.
    pub fn resolve(&self, s: &str) -> String {
        self.resolve_depth(s, 0)
    }

    fn resolve_depth(&self, s: &str, depth: usize) -> String {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            if let Some(end) = after.find("}}") {
                let name = after[..end].trim();
                match self.lookup(name) {
                    Some(v) if depth < 8 && v.contains("{{") => out.push_str(&self.resolve_depth(&v, depth + 1)),
                    Some(v) => out.push_str(&v),
                    None => out.push_str(&rest[start..start + 2 + end + 2]),
                }
                rest = &after[end + 2..];
            } else {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
        out.push_str(rest);
        out
    }

    fn lookup(&self, name: &str) -> Option<String> {
        if let Some(v) = self.map.get(name) {
            return Some(v.clone());
        }
        let r = pseudo_random();
        match name {
            "$timestamp" => Some((now_ms() / 1000).to_string()),
            "$isoTimestamp" => Some(iso_now()),
            "$randomInt" => Some((r % 1001).to_string()),
            "$guid" | "$randomUUID" => Some(uuid_like()),
            "$randomBoolean" => Some(r.is_multiple_of(2).to_string()),
            _ => None,
        }
    }

    /// Names used in `s` that cannot be resolved.
    pub fn missing(&self, s: &str) -> Vec<String> {
        let mut out = vec![];
        let mut rest = s;
        while let Some(start) = rest.find("{{") {
            let after = &rest[start + 2..];
            if let Some(end) = after.find("}}") {
                let name = after[..end].trim().to_string();
                if !self.has(&name) && !out.contains(&name) {
                    out.push(name);
                }
                rest = &after[end + 2..];
            } else {
                break;
            }
        }
        out
    }
}

fn pseudo_random() -> u64 {
    let mut x = now_ms() ^ (new_id().len() as u64) ^ (std::process::id() as u64);
    let s = new_id();
    for b in s.bytes() {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(b as u64);
    }
    x ^ (x >> 29)
}

fn uuid_like() -> String {
    let a = pseudo_random();
    let b = pseudo_random().rotate_left(17);
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        (a >> 16) as u16,
        (a & 0xfff) as u16,
        ((b >> 48) as u16 & 0x3fff) | 0x8000,
        b & 0xffff_ffff_ffff
    )
}

pub fn iso_now() -> String {
    let secs = now_ms() / 1000;
    let days = secs / 86400;
    let rem = secs % 86400;
    // civil from days (Howard Hinnant)
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60)
}
