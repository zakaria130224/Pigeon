//! Shared helpers for the tests.
#![allow(dead_code)]

use pigeon_core::git::{self, GitOp, GitResult};
use pigeon_core::model::*;
use pigeon_core::scripting::{self, Job, Outcome, Scopes};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pigeon-test-{}-{}", name, new_id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn run(dir: &Path, op: GitOp) -> GitResult {
    git::spawn(dir.to_path_buf(), op).recv().unwrap()
}

pub fn identity(dir: &Path, name: &str) {
    let r = run(dir, GitOp::SetIdentity { name: name.into(), email: format!("{}@example.com", name.to_lowercase()) });
    assert!(r.ok, "{}", r.message);
}

/// A local server that answers every request with JSON describing what it received.
pub fn echo_server() -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut data = vec![];
            let mut buf = [0u8; 4096];
            let mut header_end = None;
            while header_end.is_none() {
                let n = s.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
                header_end = data.windows(4).position(|w| w == b"\r\n\r\n");
            }
            let Some(end) = header_end else { continue };
            let head = String::from_utf8_lossy(&data[..end]).to_string();
            let length = head
                .lines()
                .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                .unwrap_or(0);
            while data.len() < end + 4 + length {
                let n = s.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
            }
            let body = String::from_utf8_lossy(&data[end + 4..]).to_string();
            let mut lines = head.lines();
            let first = lines.next().unwrap_or("").to_string();
            let headers: serde_json::Map<String, serde_json::Value> = lines
                .filter_map(|l| l.split_once(':'))
                .map(|(k, v)| (k.trim().to_lowercase(), serde_json::Value::from(v.trim())))
                .collect();
            let reply =
                serde_json::json!({"line": first, "headers": headers, "body": body, "token": "tok-123", "items": [{"id": 1}, {"id": 2}]})
                    .to_string();
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nSet-Cookie: sid=abc; Path=/\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
        }
    });
    format!("http://127.0.0.1:{}", port)
}

pub fn job(req: Request, scripts: Vec<Scripts>) -> Job {
    let mut all = scripts;
    all.push(req.scripts.clone());
    Job {
        req,
        auth_chain: vec![],
        headers: vec![],
        scripts: all,
        timeout_s: 10,
        follow_redirects: true,
        iteration: 0,
        iteration_count: 1,
        script_limit: Duration::from_secs(10),
    }
}

pub fn env_scopes() -> Scopes {
    Scopes { has_environment: true, environment_name: "Dev".into(), ..Default::default() }
}

pub fn run_scripts(url: &str, pre: &str, post: &str, scopes: &mut Scopes) -> Outcome {
    let req = Request { url: url.into(), scripts: Scripts { pre: pre.into(), post: post.into() }, ..Default::default() };
    scripting::run(&job(req, vec![]), scopes)
}

pub fn logged(o: &Outcome) -> Vec<String> {
    o.before.logs.iter().chain(o.after.logs.iter()).map(|l| l.text.clone()).collect()
}

pub fn failed(o: &Outcome) -> Vec<String> {
    o.resp.tests.iter().filter(|t| !t.1).map(|t| format!("{}: {}", t.0, t.2)).collect()
}

pub fn imported_openapi(text: &str) -> Collection {
    match pigeon_core::formats::import_json(text).unwrap() {
        pigeon_core::formats::Imported::Collection(c) => c,
        _ => panic!("expected a collection"),
    }
}

pub fn request_named<'a>(items: &'a [Item], name: &str) -> &'a Request {
    fn walk<'a>(items: &'a [Item], name: &str) -> Option<&'a Request> {
        items.iter().find_map(|i| match i {
            Item::Request(r) if r.name == name => Some(r),
            Item::Folder(f) => walk(&f.items, name),
            _ => None,
        })
    }
    walk(items, name).unwrap_or_else(|| panic!("no request named {}", name))
}
