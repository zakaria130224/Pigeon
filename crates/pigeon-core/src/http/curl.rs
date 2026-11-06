//! Reading a pasted `curl` command.

use crate::model::*;

/// Parse a pasted `curl ...` command into a request.
pub fn parse_curl(cmd: &str) -> Option<Request> {
    let cmd = cmd.replace("\\\r\n", " ").replace("\\\n", " ");
    let toks = shell_split(&cmd);
    if toks.first().map(|t| t.as_str()) != Some("curl") {
        return None;
    }
    let mut req = Request { name: "Imported cURL".into(), ..Default::default() };
    let mut method: Option<String> = None;
    let mut data: Vec<String> = vec![];
    let mut i = 1;
    while i < toks.len() {
        let t = &toks[i];
        let next = toks.get(i + 1).cloned().unwrap_or_default();
        match t.as_str() {
            "-X" | "--request" => {
                method = Some(next.to_uppercase());
                i += 1;
            }
            "-H" | "--header" => {
                if let Some((k, v)) = next.split_once(':') {
                    req.headers.push(Kv::new(k.trim(), v.trim()));
                }
                i += 1;
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-ascii" | "--data-urlencode" => {
                data.push(next);
                i += 1;
            }
            "-u" | "--user" => {
                let (u, p) = next.split_once(':').unwrap_or((&next, ""));
                req.auth = Auth { kind: AuthKind::Basic, username: u.into(), password: p.into(), ..Default::default() };
                i += 1;
            }
            "--url" => {
                req.url = next;
                i += 1;
            }
            "-A" | "--user-agent" => {
                req.headers.push(Kv::new("User-Agent", &next));
                i += 1;
            }
            "-b" | "--cookie" => {
                req.headers.push(Kv::new("Cookie", &next));
                i += 1;
            }
            s if s.starts_with('-') => {}
            s => {
                if req.url.is_empty() {
                    req.url = s.to_string();
                }
            }
        }
        i += 1;
    }
    if !data.is_empty() {
        let raw = data.join("&");
        let is_json = raw.trim_start().starts_with('{') || raw.trim_start().starts_with('[');
        req.body = Body { mode: BodyMode::Raw, raw, raw_lang: if is_json { RawLang::Json } else { RawLang::Text }, ..Default::default() };
    }
    req.method = method.unwrap_or_else(|| if data.is_empty() { "GET".into() } else { "POST".into() });
    req.params = parse_query(&req.url);
    let short: String = req.url.split('?').next().unwrap_or("").chars().take(60).collect();
    if !short.is_empty() {
        req.name = short;
    }
    Some(req)
}

fn shell_split(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut in_tok = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_tok = true;
                for c2 in chars.by_ref() {
                    if c2 == '\'' {
                        break;
                    }
                    cur.push(c2);
                }
            }
            '"' => {
                in_tok = true;
                while let Some(c2) = chars.next() {
                    match c2 {
                        '"' => break,
                        '\\' => {
                            if let Some(n) = chars.next() {
                                cur.push(n);
                            }
                        }
                        _ => cur.push(c2),
                    }
                }
            }
            '\\' => {
                in_tok = true;
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            c if c.is_whitespace() => {
                if in_tok {
                    out.push(std::mem::take(&mut cur));
                    in_tok = false;
                }
            }
            c => {
                in_tok = true;
                cur.push(c);
            }
        }
    }
    if in_tok {
        out.push(cur);
    }
    out
}
