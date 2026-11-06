mod common;

use pigeon_core::http;
use pigeon_core::model::*;

#[test]
fn variables_and_curl() {
    let env =
        Environment { id: "e".into(), name: "x".into(), values: vec![Kv::new("host", "api.test"), Kv::new("base", "https://{{host}}")] };
    let v = http::Vars::build(Some(&env), None, &[]);
    assert_eq!(v.resolve("{{base}}/a?{{missing}}"), "https://api.test/a?{{missing}}");
    assert_eq!(v.missing("{{base}} {{missing}}"), vec!["missing".to_string()]);
    let r = http::parse_curl("curl -X POST 'https://x.test/p?a=1' -H 'Accept: json' --data '{\"a\":1}'").unwrap();
    assert_eq!(r.method, "POST");
    assert_eq!(r.url, "https://x.test/p?a=1");
    assert_eq!(r.params.len(), 1);
    assert_eq!(r.body.raw_lang, RawLang::Json);
    let mut params = vec![];
    sync_params_from_url("https://x?a=1&b=2", &mut params);
    assert_eq!(params.len(), 2);
    assert_eq!(url_with_params("https://x?a=1&b=2", &params[..1]), "https://x?a=1");
}

#[test]
fn curl_snippet_puts_each_part_on_its_own_line_and_imports_back() {
    let mut req = Request { method: "POST".into(), url: "https://x.test/p".into(), ..Default::default() };
    req.headers.push(Kv::new("Accept", "json"));
    req.body.mode = BodyMode::Raw;
    req.body.raw = "it's".into();
    let vars = http::Vars::build(None, None, &[]);
    let snippet = http::curl_snippet(&http::prepare(&req, &req.auth, &vars));
    let lines: Vec<&str> = snippet.lines().collect();
    assert_eq!(lines[0], "curl --location --request POST \\");
    assert_eq!(lines[1], "  'https://x.test/p' \\");
    assert!(lines[2].starts_with("  --header 'Accept: json'"));
    let back = http::parse_curl(&snippet).unwrap();
    assert_eq!((back.method.as_str(), back.url.as_str(), back.body.raw.as_str()), ("POST", "https://x.test/p", "it's"));
}

#[test]
fn bulk_text_reads_pasted_headers() {
    let rows = kv_from_text("Accept: application/json\n\n  // X-Off : 1\nReferer: https://a.test:8080/x\nflag\n:\n", &[]);
    assert_eq!(
        rows,
        vec![
            Kv::new("Accept", "application/json"),
            Kv { enabled: false, ..Kv::new("X-Off", "1") },
            Kv::new("Referer", "https://a.test:8080/x"),
            Kv::new("flag", ""),
        ]
    );
}

#[test]
fn bulk_text_round_trips_rows() {
    let rows = vec![
        Kv { description: "paging".into(), ..Kv::new("page", "{{n}}") },
        Kv { enabled: false, ..Kv::new("a:b", " spaced") },
        Kv::new("page", "2"),
    ];
    let mut with_blank = rows.clone();
    with_blank.push(Kv::default());
    let text = kv_to_text(&with_blank);
    assert_eq!(text, "page:{{n}}\n//a:b: spaced\npage:2");
    assert_eq!(kv_from_text(&text, &rows), rows);
}

#[test]
fn bulk_edit_keeps_descriptions_while_typing() {
    let mut rows = vec![Kv { description: "who".into(), ..Kv::new("user", "1") }, Kv::new("b", "2")];
    let mut edit = BulkEdit::new(&rows);
    for text in ["use:1\nb:2", "user:9\nb:2", "//user:9\nb:2\nc:3"] {
        edit.text = text.into();
        edit.apply(&mut rows);
    }
    assert_eq!(rows[0], Kv { enabled: false, description: "who".into(), ..Kv::new("user", "9") });
    assert_eq!(rows.len(), 3);
}

#[test]
fn bulk_edit_follows_rows_changed_elsewhere() {
    let mut rows = vec![Kv::new("a", "1")];
    let mut edit = BulkEdit::new(&rows);
    edit.text = "a:2".into();
    edit.apply(&mut rows);
    edit.refresh(&rows);
    assert_eq!(edit.text, "a:2");
    sync_params_from_url("https://x?a=3&b=4", &mut rows);
    edit.refresh(&rows);
    assert_eq!(edit.text, "a:3\nb:4");
}

#[test]
fn retries_after_dropped_connection() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        // First connection: read the request, then hang up without answering.
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = s.read(&mut buf);
        drop(s);
        // Second connection: answer properly.
        let (mut s, _) = listener.accept().unwrap();
        let _ = s.read(&mut buf);
        let _ = s.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
        );
    });
    let r = Request { url: format!("http://127.0.0.1:{}/x", port), ..Default::default() };
    let resp = http::send(http::prepare(&r, &Auth::none(), &http::Vars::default()), 10, true).recv().unwrap();
    assert!(resp.error.is_none(), "{:?}", resp.error);
    assert_eq!(resp.status, 200);
    assert!(resp.body.contains("ok"));
}

#[test]
#[ignore = "needs network"]
fn live_request() {
    let c = sample_collection();
    let v = http::Vars::build(None, Some(&c), &[]);
    let r = Request { url: "{{baseUrl}}/users?_limit=2".into(), ..Default::default() };
    let p = http::prepare(&r, &Auth::none(), &v);
    let resp = http::send(p, 20, true).recv().unwrap();
    assert!(resp.error.is_none(), "{:?}", resp.error);
    assert_eq!(resp.status, 200);
    assert!(resp.is_json());
    let r = Request {
        method: "POST".into(),
        url: "{{baseUrl}}/posts".into(),
        body: Body { mode: BodyMode::Raw, raw: "{\"a\":1}".into(), ..Default::default() },
        ..Default::default()
    };
    let resp = http::send(http::prepare(&r, &Auth::none(), &v), 20, true).recv().unwrap();
    assert_eq!(resp.status, 201, "{:?}", resp.error);
}
