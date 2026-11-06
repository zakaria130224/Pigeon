//! The network call itself.

use super::{Prepared, ResponseData};
use std::time::{Duration, Instant};

pub fn send(p: Prepared, timeout_s: u64, follow_redirects: bool) -> std::sync::mpsc::Receiver<ResponseData> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let r = send_blocking(&p, timeout_s, follow_redirects);
        let _ = tx.send(r);
    });
    rx
}

pub fn send_blocking(p: &Prepared, timeout_s: u64, follow_redirects: bool) -> ResponseData {
    let start = Instant::now();
    let wire = with_defaults(p);
    let p = &wire;
    let mut out = ResponseData { final_url: p.url.clone(), sent: Some(wire.clone()), ..Default::default() };
    if p.url.is_empty() {
        out.error = Some("Enter a URL to send a request.".into());
        return out;
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .allow_non_standard_methods(true)
        .timeout_global(Some(Duration::from_secs(timeout_s.max(1))))
        .max_redirects(if follow_redirects { 10 } else { 0 })
        .max_redirects_will_error(false)
        .user_agent("PigeonRuntime/1.0")
        .build()
        .into();
    // Servers and proxies sometimes drop a connection ("connection reset by peer").
    // That is almost always temporary, so try again a couple of times with a fresh connection.
    const ATTEMPTS: usize = 3;
    for attempt in 1..=ATTEMPTS {
        out.attempts = attempt;
        match attempt_once(&agent, p) {
            Ok((status, status_text, headers, bytes)) => {
                out.status = status;
                out.status_text = status_text;
                out.headers = headers;
                out.size = bytes.len();
                out.body = String::from_utf8_lossy(&bytes).to_string();
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes)
                    && (v.is_object() || v.is_array())
                {
                    out.pretty = serde_json::to_string_pretty(&v).ok();
                }
                out.error = None;
                break;
            }
            Err(e) => {
                let retry = is_transient(&e) && attempt < ATTEMPTS;
                out.error = Some(friendly_error(&e));
                if !retry {
                    break;
                }
                std::thread::sleep(Duration::from_millis(250 * attempt as u64));
            }
        }
    }
    out.time_ms = start.elapsed().as_millis() as u64;
    out
}

type RawResponse = (u16, String, Vec<(String, String)>, Vec<u8>);

/// Add the friendly default headers most API clients send; user headers win.
fn with_defaults(p: &Prepared) -> Prepared {
    let mut w = p.clone();
    let has = |n: &str| p.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(n));
    if !has("user-agent") {
        w.headers.push(("User-Agent".into(), "PigeonRuntime/1.0".into()));
    }
    if !has("accept") {
        w.headers.push(("Accept".into(), "*/*".into()));
    }
    if !has("cache-control") {
        w.headers.push(("Cache-Control".into(), "no-cache".into()));
    }
    w
}

fn attempt_once(agent: &ureq::Agent, p: &Prepared) -> Result<RawResponse, ureq::Error> {
    let mut builder = ureq::http::Request::builder().method(p.method.as_str()).uri(p.url.as_str());
    for (k, v) in &p.headers {
        builder = builder.header(k.as_str(), v.as_str());
    }
    let resp = match p.body.as_ref().filter(|b| !b.is_empty()) {
        Some(b) => agent.run(builder.body(b.clone()).map_err(|e| ureq::Error::BadUri(e.to_string()))?),
        // No body at all (rather than an empty one) for GET-style requests.
        None => agent.run(builder.body(()).map_err(|e| ureq::Error::BadUri(e.to_string()))?),
    };
    let mut resp = resp?;
    let status = resp.status().as_u16();
    let status_text = resp.status().canonical_reason().unwrap_or("").to_string();
    let headers = resp.headers().iter().map(|(k, v)| (k.to_string(), String::from_utf8_lossy(v.as_bytes()).to_string())).collect();
    let bytes = resp.body_mut().with_config().limit(50 * 1024 * 1024).read_to_vec()?;
    Ok((status, status_text, headers, bytes))
}

fn is_transient(e: &ureq::Error) -> bool {
    use std::io::ErrorKind::*;
    match e {
        ureq::Error::Io(io) => matches!(io.kind(), ConnectionReset | ConnectionAborted | BrokenPipe | UnexpectedEof | Interrupted),
        ureq::Error::ConnectionFailed => true,
        _ => false,
    }
}

fn friendly_error(e: &ureq::Error) -> String {
    let s = e.to_string();
    let hint = match e {
        ureq::Error::HostNotFound => "The server name could not be found. Check the URL.",
        ureq::Error::Timeout(_) => "The server took too long to answer.",
        ureq::Error::ConnectionFailed => "Could not connect to the server.",
        ureq::Error::Io(io) if is_transient(e) && io.kind() != std::io::ErrorKind::Interrupted => {
            "The server kept closing the connection (tried 3 times). Check the URL uses the right http/https and port, or that a VPN/proxy isn't blocking it."
        }
        _ => "",
    };
    if hint.is_empty() { s } else { format!("{} ({})", hint, s) }
}
