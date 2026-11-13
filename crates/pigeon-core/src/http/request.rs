//! Turning a saved request into exactly what goes on the wire.

use super::{Prepared, Vars};
use crate::model::*;

/// Pick the effective auth for a request (walking up through folders and collection).
pub fn effective_auth(req: &Request, chain: &[Auth]) -> Auth {
    if req.auth.kind != AuthKind::Inherit {
        return req.auth.clone();
    }
    for a in chain {
        if a.kind != AuthKind::Inherit {
            return a.clone();
        }
    }
    Auth::none()
}

/// Put the collection's headers in front of the request's own, leaving out any the request
/// already sets. Names are compared after variables are filled in, ignoring case.
pub fn inherit_headers(req: &mut Request, inherited: &[Kv], vars: &Vars) {
    let name = |h: &Kv| vars.resolve(h.key.trim()).to_ascii_lowercase();
    let on = |h: &&Kv| h.enabled && !h.key.trim().is_empty();
    let own: Vec<String> = req.headers.iter().filter(on).map(name).collect();
    let mut merged: Vec<Kv> = inherited.iter().filter(on).filter(|h| !own.contains(&name(h))).cloned().collect();
    merged.append(&mut req.headers);
    req.headers = merged;
}

pub fn base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn prepare(req: &Request, auth: &Auth, vars: &Vars) -> Prepared {
    let mut url = vars.resolve(req.url.trim());
    if !url.is_empty() && !url.contains("://") {
        url = format!("http://{}", url);
    }
    let mut headers: Vec<(String, String)> = req
        .headers
        .iter()
        .filter(|h| h.enabled && !h.key.trim().is_empty())
        .map(|h| (vars.resolve(h.key.trim()), vars.resolve(&h.value)))
        .collect();
    let has = |hs: &Vec<(String, String)>, n: &str| hs.iter().any(|(k, _)| k.eq_ignore_ascii_case(n));
    match auth.kind {
        AuthKind::Bearer if !has(&headers, "authorization") => {
            headers.push(("Authorization".into(), format!("Bearer {}", vars.resolve(&auth.token))));
        }
        AuthKind::Basic if !has(&headers, "authorization") => {
            let raw = format!("{}:{}", vars.resolve(&auth.username), vars.resolve(&auth.password));
            headers.push(("Authorization".into(), format!("Basic {}", base64(raw.as_bytes()))));
        }
        AuthKind::ApiKey if !auth.key.is_empty() => {
            let k = vars.resolve(&auth.key);
            let v = vars.resolve(&auth.value);
            if auth.in_query {
                let sep = if url.contains('?') { '&' } else { '?' };
                url = format!("{}{}{}={}", url, sep, percent_encode(&k), percent_encode(&v));
            } else if !has(&headers, &k) {
                headers.push((k, v));
            }
        }
        _ => {}
    }
    let body = match req.body.mode {
        BodyMode::None => None,
        BodyMode::Raw => {
            if !has(&headers, "content-type") {
                headers.push(("Content-Type".into(), req.body.raw_lang.content_type().into()));
            }
            Some(vars.resolve(&req.body.raw).into_bytes())
        }
        BodyMode::UrlEncoded => {
            if !has(&headers, "content-type") {
                headers.push(("Content-Type".into(), "application/x-www-form-urlencoded".into()));
            }
            let s: Vec<String> = req
                .body
                .urlencoded
                .iter()
                .filter(|k| k.enabled && !k.key.is_empty())
                .map(|k| format!("{}={}", percent_encode(&vars.resolve(&k.key)), percent_encode(&vars.resolve(&k.value))))
                .collect();
            Some(s.join("&").into_bytes())
        }
        BodyMode::FormData => {
            let boundary = format!("----PigeonBoundary{}", new_id());
            headers.retain(|(k, _)| !k.eq_ignore_ascii_case("content-type"));
            headers.push(("Content-Type".into(), format!("multipart/form-data; boundary={}", boundary)));
            let mut b = String::new();
            for kv in req.body.formdata.iter().filter(|k| k.enabled && !k.key.is_empty()) {
                b.push_str(&format!(
                    "--{}\r\nContent-Disposition: form-data; name=\"{}\"\r\n\r\n{}\r\n",
                    boundary,
                    vars.resolve(&kv.key),
                    vars.resolve(&kv.value)
                ));
            }
            b.push_str(&format!("--{}--\r\n", boundary));
            Some(b.into_bytes())
        }
    };
    Prepared { method: req.method.to_uppercase(), url: encode_url_loose(&url), headers, body }
}
