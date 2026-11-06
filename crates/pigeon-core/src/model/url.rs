//! URL and query string helpers.

use super::Kv;

/// Parse the query portion of a URL into key/values (without decoding, as typed).
pub fn parse_query(url: &str) -> Vec<Kv> {
    let Some(q) = url.split_once('?').map(|x| x.1) else { return vec![] };
    let q = q.split('#').next().unwrap_or("");
    if q.is_empty() {
        return vec![];
    }
    q.split('&')
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            Kv::new(k, v)
        })
        .collect()
}

/// Rewrite the query portion of `url` from enabled params.
pub fn url_with_params(url: &str, params: &[Kv]) -> String {
    let base = url.split_once('?').map(|x| x.0).unwrap_or(url);
    let q: Vec<String> = params
        .iter()
        .filter(|p| p.enabled && !(p.key.is_empty() && p.value.is_empty()))
        .map(|p| if p.value.is_empty() { p.key.clone() } else { format!("{}={}", p.key, p.value) })
        .collect();
    if q.is_empty() { base.to_string() } else { format!("{}?{}", base, q.join("&")) }
}

/// Merge URL query into params: keep disabled params, replace enabled ones with what the URL says.
pub fn sync_params_from_url(url: &str, params: &mut Vec<Kv>) {
    let parsed = parse_query(url);
    let mut out: Vec<Kv> = Vec::new();
    let mut pi = parsed.into_iter();
    for p in params.iter() {
        if !p.enabled {
            out.push(p.clone());
        } else if let Some(mut n) = pi.next() {
            n.description = p.description.clone();
            out.push(n);
        }
    }
    out.extend(pi);
    *params = out;
}

pub fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Encode characters that are not valid in a URL but leave existing structure alone.
pub fn encode_url_loose(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            ' ' => out.push_str("%20"),
            '"' => out.push_str("%22"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '`' => out.push_str("%60"),
            '{' => out.push_str("%7B"),
            '}' => out.push_str("%7D"),
            '|' => out.push_str("%7C"),
            '\\' => out.push_str("%5C"),
            '^' => out.push_str("%5E"),
            c if c.is_ascii() && !c.is_control() => out.push(c),
            c => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    out
}

pub const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
